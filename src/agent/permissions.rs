//! Permission system for LocalClaw agents.
//!
//! Provides permission levels, request tracking, and UI notification signals
//! for approval workflows.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use dioxus::prelude::{Signal, Writable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;
use tokio::time::{sleep, Duration, Instant};

/// Permission level for agent operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// Can only read files/system info
    ReadOnly,
    /// Can write files (creates/modifies)
    WriteFile,
    /// Can read/write files in allowed directories
    ReadWrite,
    /// Can execute safe commands (ls, cat, etc.)
    ExecuteSafe,
    /// Can execute any command (requires approval)
    ExecuteUnsafe,
    /// Can make network requests
    Network,
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionLevel::ReadOnly => write!(f, "read"),
            PermissionLevel::WriteFile => write!(f, "write"),
            PermissionLevel::ReadWrite => write!(f, "rw"),
            PermissionLevel::ExecuteSafe => write!(f, "exec"),
            PermissionLevel::ExecuteUnsafe => write!(f, "exec!"),
            PermissionLevel::Network => write!(f, "net"),
        }
    }
}

impl PermissionLevel {
    fn rank(self) -> u8 {
        match self {
            PermissionLevel::ReadOnly => 0,
            PermissionLevel::WriteFile => 1,
            PermissionLevel::ReadWrite => 2,
            PermissionLevel::ExecuteSafe => 3,
            PermissionLevel::ExecuteUnsafe => 4,
            PermissionLevel::Network => 5,
        }
    }
    
    /// Human-readable label for UI
    pub fn label(&self) -> &'static str {
        match self {
            PermissionLevel::ReadOnly => "Lecture seule",
            PermissionLevel::WriteFile => "Écriture fichier",
            PermissionLevel::ReadWrite => "Lecture/Écriture",
            PermissionLevel::ExecuteSafe => "Commandes sûres",
            PermissionLevel::ExecuteUnsafe => "Commandes dangereuses",
            PermissionLevel::Network => "Réseau",
        }
    }
    
    /// Icon for UI
    pub fn icon(&self) -> &'static str {
        match self {
            PermissionLevel::ReadOnly => "👁️",
            PermissionLevel::WriteFile => "📝",
            PermissionLevel::ReadWrite => "📂",
            PermissionLevel::ExecuteSafe => "⚡",
            PermissionLevel::ExecuteUnsafe => "⚠️",
            PermissionLevel::Network => "🌐",
        }
    }
}

/// Request for a permission decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: Uuid,
    pub tool_name: String,
    pub operation: String,
    pub target: String,
    pub level: PermissionLevel,
    pub params: Value,
    pub timestamp: DateTime<Utc>,
}

/// Policy configuration for permission checks.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub allow_network: bool,
    pub allowed_directories: Vec<PathBuf>,
    pub blocked_directories: Vec<PathBuf>,
    pub allowed_commands: Vec<String>,
    pub require_approval_for: Vec<String>,
}

/// Result of a permission request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionResult {
    Approved,
    Denied,
    Pending,
}

/// Decision event for UI notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Approved,
    Denied,
}

/// Notification payload for UI signals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionNotification {
    pub request_id: Uuid,
    pub decision: PermissionDecision,
    pub timestamp: DateTime<Utc>,
}

/// Errors raised by the permission manager.
#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("Permission request not found: {0}")]
    NotFound(Uuid),
    #[error("Permission request already decided: {0}")]
    AlreadyDecided(Uuid),
}

/// Dioxus signals for UI notification.
#[derive(Clone)]
pub struct PermissionSignals {
    pub pending_requests: Signal<Vec<PermissionRequest>>,
    pub last_decision: Signal<Option<PermissionNotification>>,
}

/// Permission manager for request tracking and decisions.
pub struct PermissionManager {
    pending: Arc<Mutex<Vec<PermissionRequest>>>,
    approved: Arc<Mutex<HashSet<Uuid>>>,
    denied: Arc<Mutex<HashSet<Uuid>>>,
    default_level: PermissionLevel,
    signals: PermissionSignals,
}

impl PermissionManager {
    pub fn new(default_level: PermissionLevel) -> Self {
        let pending = Signal::new(Vec::new());
        let last_decision = Signal::new(None);
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
            approved: Arc::new(Mutex::new(HashSet::new())),
            denied: Arc::new(Mutex::new(HashSet::new())),
            default_level,
            signals: PermissionSignals {
                pending_requests: pending,
                last_decision,
            },
        }
    }

    /// Requests permission for a specific operation.
    pub async fn request_permission(&self, request: PermissionRequest) -> PermissionResult {
        if self.check_permission(&request.tool_name, request.level) {
            self.approved
                .lock()
                .expect("approved mutex poisoned")
                .insert(request.id);
            return PermissionResult::Approved;
        }

        self.pending
            .lock()
            .expect("pending mutex poisoned")
            .push(request);
        self.sync_pending_signal();
        PermissionResult::Pending
    }

    /// Approves a pending permission request.
    pub async fn approve(&self, request_id: Uuid) -> Result<(), PermissionError> {
        self.ensure_not_decided(request_id)?;
        if !self.remove_pending(request_id) {
            return Err(PermissionError::NotFound(request_id));
        }
        self.approved
            .lock()
            .expect("approved mutex poisoned")
            .insert(request_id);
        self.sync_pending_signal();
        self.emit_decision(request_id, PermissionDecision::Approved);
        Ok(())
    }

    /// Denies a pending permission request.
    pub async fn deny(&self, request_id: Uuid) -> Result<(), PermissionError> {
        self.ensure_not_decided(request_id)?;
        if !self.remove_pending(request_id) {
            return Err(PermissionError::NotFound(request_id));
        }
        self.denied
            .lock()
            .expect("denied mutex poisoned")
            .insert(request_id);
        self.sync_pending_signal();
        self.emit_decision(request_id, PermissionDecision::Denied);
        Ok(())
    }

    /// Checks whether a permission level is allowed by default.
    pub fn check_permission(&self, _tool: &str, level: PermissionLevel) -> bool {
        level.rank() <= self.default_level.rank()
    }

    /// Returns the decision for a request if it has been decided.
    pub fn decision_for(&self, request_id: Uuid) -> Option<PermissionDecision> {
        let approved = self.approved.lock().expect("approved mutex poisoned");
        if approved.contains(&request_id) {
            return Some(PermissionDecision::Approved);
        }
        drop(approved);

        let denied = self.denied.lock().expect("denied mutex poisoned");
        if denied.contains(&request_id) {
            return Some(PermissionDecision::Denied);
        }
        None
    }

    /// Waits for a permission decision or times out.
    pub async fn wait_for_decision(
        &self,
        request_id: Uuid,
        timeout: Duration,
    ) -> Option<PermissionDecision> {
        let start = Instant::now();
        loop {
            if let Some(decision) = self.decision_for(request_id) {
                return Some(decision);
            }

            if start.elapsed() >= timeout {
                return None;
            }

            sleep(Duration::from_millis(200)).await;
        }
    }

    /// Returns a snapshot of pending permission requests.
    pub fn get_pending_requests(&self) -> Vec<PermissionRequest> {
        self.pending
            .lock()
            .expect("pending mutex poisoned")
            .clone()
    }

    /// Access Dioxus signals for UI notifications.
    pub fn signals(&self) -> PermissionSignals {
        self.signals.clone()
    }

    fn remove_pending(&self, request_id: Uuid) -> bool {
        let mut pending = self.pending.lock().expect("pending mutex poisoned");
        let before = pending.len();
        pending.retain(|request| request.id != request_id);
        before != pending.len()
    }

    fn ensure_not_decided(&self, request_id: Uuid) -> Result<(), PermissionError> {
        let approved = self.approved.lock().expect("approved mutex poisoned");
        let denied = self.denied.lock().expect("denied mutex poisoned");
        if approved.contains(&request_id) || denied.contains(&request_id) {
            return Err(PermissionError::AlreadyDecided(request_id));
        }
        Ok(())
    }

    fn sync_pending_signal(&self) {
        let pending = self
            .pending
            .lock()
            .expect("pending mutex poisoned")
            .clone();
        // Clone the signal to get a mutable reference
        let mut signal = self.signals.pending_requests.clone();
        signal.set(pending);
    }

    fn emit_decision(&self, request_id: Uuid, decision: PermissionDecision) {
        let notification = PermissionNotification {
            request_id,
            decision,
            timestamp: Utc::now(),
        };
        // Clone the signal to get a mutable reference
        let mut signal = self.signals.last_decision.clone();
        signal.set(Some(notification));
    }
}
