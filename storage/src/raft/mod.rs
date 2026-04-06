use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

/// Raft节点ID
type NodeId = String;

/// Raft日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub command: Vec<u8>,
}

/// Raft消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// 请求投票
    RequestVote(RequestVoteArgs),
    /// 请求投票响应
    RequestVoteResponse(RequestVoteReply),
    /// 追加日志条目
    AppendEntries(AppendEntriesArgs),
    /// 追加日志条目响应
    AppendEntriesResponse(AppendEntriesReply),
    /// 客户端请求
    ClientRequest(ClientRequestArgs),
    /// 客户端响应
    ClientResponse(ClientResponseReply),
}

/// 请求投票参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteArgs {
    pub term: u64,
    pub candidate_id: NodeId,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

/// 请求投票响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteReply {
    pub term: u64,
    pub vote_granted: bool,
}

/// 追加日志条目参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesArgs {
    pub term: u64,
    pub leader_id: NodeId,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

/// 追加日志条目响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesReply {
    pub term: u64,
    pub success: bool,
    pub match_index: u64,
}

/// 客户端请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRequestArgs {
    pub command: Vec<u8>,
}

/// 客户端响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientResponseReply {
    pub success: bool,
    pub leader_id: Option<NodeId>,
    pub result: Option<Vec<u8>>,
}

/// Raft节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

/// Raft配置
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// 节点ID
    pub node_id: NodeId,
    /// 其他节点地址
    pub peers: Vec<NodeId>,
    /// 选举超时最小值（毫秒）
    pub election_timeout_min: u64,
    /// 选举超时最大值（毫秒）
    pub election_timeout_max: u64,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: "node-1".to_string(),
            peers: Vec::new(),
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
        }
    }
}

/// Raft节点
pub struct RaftNode {
    config: RaftConfig,
    state: Arc<RwLock<RaftState>>,
    current_term: Arc<RwLock<u64>>,
    voted_for: Arc<RwLock<Option<NodeId>>>,
    log: Arc<RwLock<Vec<LogEntry>>>,
    commit_index: Arc<RwLock<u64>>,
    last_applied: Arc<RwLock<u64>>,
    next_index: Arc<RwLock<HashMap<NodeId, u64>>>,
    match_index: Arc<RwLock<HashMap<NodeId, u64>>>,
    last_heartbeat: Arc<RwLock<Instant>>,
    tx: mpsc::Sender<RaftMessage>,
}

impl RaftNode {
    pub fn new(config: RaftConfig) -> (Self, mpsc::Receiver<RaftMessage>) {
        let (tx, rx) = mpsc::channel(1000);
        let node_id = config.node_id.clone();
        let peers = config.peers.clone();

        let node = Self {
            config,
            state: Arc::new(RwLock::new(RaftState::Follower)),
            current_term: Arc::new(RwLock::new(0)),
            voted_for: Arc::new(RwLock::new(None)),
            log: Arc::new(RwLock::new(Vec::new())),
            commit_index: Arc::new(RwLock::new(0)),
            last_applied: Arc::new(RwLock::new(0)),
            next_index: Arc::new(RwLock::new(
                peers.iter().map(|p| (p.clone(), 1)).collect()
            )),
            match_index: Arc::new(RwLock::new(
                peers.iter().map(|p| (p.clone(), 0)).collect()
            )),
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
            tx,
        };

        (node, rx)
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<RaftMessage>) -> Result<()> {
        info!("Raft node {} starting...", self.config.node_id);

        let election_timeout = self.random_election_timeout();
        let mut election_timer = interval(Duration::from_millis(election_timeout));
        let mut heartbeat_timer = interval(Duration::from_millis(self.config.heartbeat_interval));

        loop {
            let state = *self.state.read().await;

            match state {
                RaftState::Follower | RaftState::Candidate => {
                    tokio::select! {
                        _ = election_timer.tick() => {
                            self.handle_election_timeout().await?;
                        }
                        Some(msg) = rx.recv() => {
                            self.handle_message(msg).await?;
                        }
                    }
                }
                RaftState::Leader => {
                    tokio::select! {
                        _ = heartbeat_timer.tick() => {
                            self.send_heartbeats().await?;
                        }
                        Some(msg) = rx.recv() => {
                            self.handle_message(msg).await?;
                        }
                    }
                }
            }
        }
    }

    async fn handle_election_timeout(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if *state == RaftState::Leader {
            return Ok(());
        }

        // Become candidate
        *state = RaftState::Candidate;
        drop(state);

        let mut current_term = self.current_term.write().await;
        *current_term += 1;
        let term = *current_term;
        drop(current_term);

        let mut voted_for = self.voted_for.write().await;
        *voted_for = Some(self.config.node_id.clone());
        drop(voted_for);

        info!(
            "Node {} became candidate for term {}",
            self.config.node_id, term
        );

        // Request votes from all peers
        self.request_votes(term).await?;

        Ok(())
    }

    async fn request_votes(&self, term: u64) -> Result<()> {
        let log = self.log.read().await;
        let last_log_index = log.len() as u64;
        let last_log_term = log.last().map(|e| e.term).unwrap_or(0);
        drop(log);

        let args = RequestVoteArgs {
            term,
            candidate_id: self.config.node_id.clone(),
            last_log_index,
            last_log_term,
        };

        let _message = RaftMessage::RequestVote(args);

        // Send vote requests to all peers
        for peer in &self.config.peers {
            debug!("Requesting vote from {} for term {}", peer, term);
            // In a real implementation, this would send the message over the network
        }

        Ok(())
    }

    async fn send_heartbeats(&self) -> Result<()> {
        let state = self.state.read().await;
        if *state != RaftState::Leader {
            return Ok(());
        }
        drop(state);

        let current_term = *self.current_term.read().await;
        let commit_index = *self.commit_index.read().await;

        for peer in &self.config.peers {
            let next_index = self.next_index.read().await;
            let prev_log_index = next_index.get(peer).copied().unwrap_or(1) - 1;
            drop(next_index);

            let log = self.log.read().await;
            let prev_log_term = if prev_log_index > 0 {
                log.get((prev_log_index - 1) as usize)
                    .map(|e| e.term)
                    .unwrap_or(0)
            } else {
                0
            };

            // Get entries to send
            let entries: Vec<LogEntry> = log
                .iter()
                .skip(prev_log_index as usize)
                .cloned()
                .collect();
            drop(log);

            let args = AppendEntriesArgs {
                term: current_term,
                leader_id: self.config.node_id.clone(),
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: commit_index,
            };

            let _message = RaftMessage::AppendEntries(args);
            debug!("Sending heartbeat to {}", peer);
            // In a real implementation, this would send the message over the network
        }

        Ok(())
    }

    async fn handle_message(&self, msg: RaftMessage) -> Result<()> {
        match msg {
            RaftMessage::RequestVote(args) => {
                self.handle_request_vote(args).await?;
            }
            RaftMessage::RequestVoteResponse(reply) => {
                self.handle_request_vote_response(reply).await?;
            }
            RaftMessage::AppendEntries(args) => {
                self.handle_append_entries(args).await?;
            }
            RaftMessage::AppendEntriesResponse(reply) => {
                self.handle_append_entries_response(reply).await?;
            }
            RaftMessage::ClientRequest(args) => {
                self.handle_client_request(args).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_request_vote(&self, args: RequestVoteArgs) -> Result<()> {
        let mut current_term = self.current_term.write().await;

        if args.term < *current_term {
            // Reject vote
            let _reply = RequestVoteReply {
                term: *current_term,
                vote_granted: false,
            };
            // Send reply
            return Ok(());
        }

        if args.term > *current_term {
            *current_term = args.term;
            let mut state = self.state.write().await;
            *state = RaftState::Follower;
            let mut voted_for = self.voted_for.write().await;
            *voted_for = None;
        }

        let voted_for = self.voted_for.read().await;
        let can_vote = voted_for.is_none() || voted_for.as_ref() == Some(&args.candidate_id);
        drop(voted_for);

        if can_vote {
            let log = self.log.read().await;
            let last_log_index = log.len() as u64;
            let last_log_term = log.last().map(|e| e.term).unwrap_or(0);
            drop(log);

            let log_is_up_to_date = args.last_log_term > last_log_term
                || (args.last_log_term == last_log_term && args.last_log_index >= last_log_index);

            if log_is_up_to_date {
                let mut voted_for = self.voted_for.write().await;
                *voted_for = Some(args.candidate_id.clone());
                drop(voted_for);

                let _reply = RequestVoteReply {
                    term: *current_term,
                    vote_granted: true,
                };
                // Send reply
                info!("Voted for {} in term {}", args.candidate_id, args.term);
            }
        }

        Ok(())
    }

    async fn handle_request_vote_response(&self, reply: RequestVoteReply) -> Result<()> {
        let current_term = *self.current_term.read().await;

        if reply.term > current_term {
            let mut current_term = self.current_term.write().await;
            *current_term = reply.term;
            let mut state = self.state.write().await;
            *state = RaftState::Follower;
            let mut voted_for = self.voted_for.write().await;
            *voted_for = None;
            return Ok(());
        }

        if reply.vote_granted {
            // Count votes and become leader if majority
            let state = self.state.read().await;
            if *state == RaftState::Candidate {
                // In a real implementation, count votes and check for majority
                // For now, just become leader if we get any vote
                drop(state);
                let mut state = self.state.write().await;
                *state = RaftState::Leader;
                info!("Node {} became leader for term {}", self.config.node_id, current_term);

                // Initialize leader state
                let mut next_index = self.next_index.write().await;
                let log_len = self.log.read().await.len() as u64 + 1;
                for peer in &self.config.peers {
                    next_index.insert(peer.clone(), log_len);
                }
                drop(next_index);

                let mut match_index = self.match_index.write().await;
                for peer in &self.config.peers {
                    match_index.insert(peer.clone(), 0);
                }
            }
        }

        Ok(())
    }

    async fn handle_append_entries(&self, args: AppendEntriesArgs) -> Result<()> {
        let mut current_term = self.current_term.write().await;

        if args.term < *current_term {
            let _reply = AppendEntriesReply {
                term: *current_term,
                success: false,
                match_index: 0,
            };
            // Send reply
            return Ok(());
        }

        // Reset election timeout
        let mut last_heartbeat = self.last_heartbeat.write().await;
        *last_heartbeat = Instant::now();
        drop(last_heartbeat);

        if args.term > *current_term {
            *current_term = args.term;
            let mut state = self.state.write().await;
            *state = RaftState::Follower;
            let mut voted_for = self.voted_for.write().await;
            *voted_for = None;
        }

        drop(current_term);

        // Check log consistency
        let mut log = self.log.write().await;
        if args.prev_log_index > 0 {
            if args.prev_log_index > log.len() as u64 {
                let _reply = AppendEntriesReply {
                    term: args.term,
                    success: false,
                    match_index: 0,
                };
                // Send reply
                return Ok(());
            }

            if args.prev_log_index > 0 {
                let entry = &log[(args.prev_log_index - 1) as usize];
                if entry.term != args.prev_log_term {
                    // Log inconsistency, truncate log
                    log.truncate((args.prev_log_index - 1) as usize);
                    let _reply = AppendEntriesReply {
                        term: args.term,
                        success: false,
                        match_index: 0,
                    };
                    // Send reply
                    return Ok(());
                }
            }
        }

        // Save entries count before consuming
        let entries_count = args.entries.len() as u64;

        // Append new entries
        for entry in args.entries {
            if entry.index <= log.len() as u64 {
                // Check if entry matches
                if let Some(existing) = log.get((entry.index - 1) as usize) {
                    if existing.term != entry.term {
                        // Conflict, truncate and append
                        log.truncate((entry.index - 1) as usize);
                        log.push(entry);
                    }
                }
            } else {
                log.push(entry);
            }
        }

        // Update commit index
        if args.leader_commit > *self.commit_index.read().await {
            let mut commit_index = self.commit_index.write().await;
            *commit_index = args.leader_commit.min(log.len() as u64);
        }

        drop(log);

        let _reply = AppendEntriesReply {
            term: args.term,
            success: true,
            match_index: args.prev_log_index + entries_count,
        };
        // Send reply

        Ok(())
    }

    async fn handle_append_entries_response(&self, reply: AppendEntriesReply) -> Result<()> {
        let current_term = *self.current_term.read().await;

        if reply.term > current_term {
            let mut current_term = self.current_term.write().await;
            *current_term = reply.term;
            let mut state = self.state.write().await;
            *state = RaftState::Follower;
            let mut voted_for = self.voted_for.write().await;
            *voted_for = None;
            return Ok(());
        }

        if reply.success {
            // Update next_index and match_index
            let mut _next_index = self.next_index.write().await;
            let mut _match_index = self.match_index.write().await;
            // Update for the specific peer
            // In a real implementation, track which peer sent the response
        } else {
            // Decrement next_index and retry
            let mut _next_index = self.next_index.write().await;
            // Decrement for the specific peer
        }

        // Check if we can advance commit_index
        let match_index = self.match_index.read().await;
        let mut match_indices: Vec<u64> = match_index.values().copied().collect();
        match_indices.push(self.log.read().await.len() as u64);
        match_indices.sort_unstable();

        let majority = (self.config.peers.len() + 1) / 2 + 1;
        if match_indices.len() >= majority {
            let new_commit_index = match_indices[match_indices.len() - majority];
            let current_commit = *self.commit_index.read().await;

            if new_commit_index > current_commit {
                let log = self.log.read().await;
                if let Some(entry) = log.get((new_commit_index - 1) as usize) {
                    if entry.term == current_term {
                        let mut commit_index = self.commit_index.write().await;
                        *commit_index = new_commit_index;
                        info!("Advanced commit_index to {}", new_commit_index);
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_client_request(&self, args: ClientRequestArgs) -> Result<()> {
        let state = self.state.read().await;

        if *state != RaftState::Leader {
            // Redirect to leader
            let _reply = ClientResponseReply {
                success: false,
                leader_id: None, // Would need to track leader
                result: None,
            };
            // Send reply
            return Ok(());
        }
        drop(state);

        // Append to log
        let mut log = self.log.write().await;
        let index = log.len() as u64 + 1;
        let term = *self.current_term.read().await;

        let entry = LogEntry {
            index,
            term,
            command: args.command,
        };

        log.push(entry);
        info!("Appended entry at index {} in term {}", index, term);
        drop(log);

        // Replicate to followers
        self.send_heartbeats().await?;

        Ok(())
    }

    fn random_election_timeout(&self) -> u64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(self.config.election_timeout_min..=self.config.election_timeout_max)
    }

    pub fn get_tx(&self) -> mpsc::Sender<RaftMessage> {
        self.tx.clone()
    }

    pub async fn is_leader(&self) -> bool {
        *self.state.read().await == RaftState::Leader
    }

    pub async fn get_state(&self) -> RaftState {
        *self.state.read().await
    }

    pub async fn get_term(&self) -> u64 {
        *self.current_term.read().await
    }

    pub async fn get_commit_index(&self) -> u64 {
        *self.commit_index.read().await
    }
}

/// Raft状态机接口
#[async_trait::async_trait]
pub trait StateMachine: Send + Sync {
    async fn apply(&mut self, command: &[u8]) -> Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_state() {
        assert_ne!(RaftState::Follower, RaftState::Leader);
        assert_eq!(RaftState::Candidate, RaftState::Candidate);
    }

    #[test]
    fn test_log_entry() {
        let entry = LogEntry {
            index: 1,
            term: 1,
            command: vec![1, 2, 3],
        };
        assert_eq!(entry.index, 1);
        assert_eq!(entry.term, 1);
    }

    #[tokio::test]
    async fn test_raft_node_new() {
        let config = RaftConfig::default();
        let (node, _rx) = RaftNode::new(config);
        assert!(!node.is_leader().await);
    }
}
