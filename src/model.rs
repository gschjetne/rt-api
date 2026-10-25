//! Request and response types of the JSON API.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

pub use crate::doc::{DecisionContent, Segment};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

macro_rules! api_types {
    ($($item:item)*) => {
        $(
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            #[cfg_attr(feature = "openapi", derive(ToSchema))]
            $item
        )*
    };
}

api_types! {
    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum DecisionKind {
        GoNogo,
        Choice,
        Text,
    }

    pub struct Health {
        pub service: String,
        pub version: String,
        pub api: u32,
        /// People sign in through a code host here (a cloud host), so `rt init` can connect
        /// a repository to it.
        #[serde(default)]
        pub sign_in: bool,
    }

    pub struct ErrorBody {
        pub error: ErrorDetail,
    }

    pub struct ErrorDetail {
        /// A stable machine-readable code such as `not_found` or `invalid_document`.
        pub code: String,
        pub message: String,
    }

    pub struct Project {
        pub name: String,
        /// Where the repository lives on the daemon's machine.
        pub repo_path: Option<String>,
        /// The repository on a code host, for projects registered by installing the app.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub repo: Option<RepoRef>,
        /// The app was uninstalled from the repository; its sessions stay readable.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub archived: bool,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub created_at: Timestamp,
    }

    /// A repository on a code host.
    pub struct RepoRef {
        /// Such as `github`.
        pub provider: String,
        /// The host's id for the repository, stable across renames.
        pub id: u64,
        /// Such as `octo-org/app`.
        pub full_name: String,
        /// The app installation that registered it.
        pub installation: u64,
        pub private: bool,
    }

    /// A person on the code host.
    #[derive(Eq, Hash)]
    pub struct UserRef {
        /// The host's id for the account, stable across renames.
        pub id: String,
        pub login: String,
    }

    pub struct CreateProject {
        pub name: String,
        pub repo_path: Option<String>,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum WorkPackageStatus {
        Planning,
        Building,
        Reviewing,
        Accepted,
        Integrated,
        Deployed,
    }

    pub struct WorkPackage {
        pub project: String,
        pub name: String,
        pub title: Option<String>,
        pub status: WorkPackageStatus,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub updated_at: Timestamp,
    }

    pub struct CreateWorkPackage {
        pub name: String,
        pub title: Option<String>,
        pub status: Option<WorkPackageStatus>,
    }

    pub struct UpdateWorkPackage {
        pub title: Option<String>,
        pub status: Option<WorkPackageStatus>,
    }

    pub struct WorkPackageDetail {
        pub work_package: WorkPackage,
        pub sessions: Vec<SessionSummary>,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum SessionStatus {
        Open,
        Closed,
    }

    pub struct DecisionCounts {
        /// Decisions not withdrawn.
        pub active: u32,
        pub withdrawn: u32,
        /// Active decisions with a sent answer to their current version.
        pub answered: u32,
        pub drafts: u32,
        pub stale: u32,
        pub recorded_from_chat: u32,
    }

    pub struct SessionSummary {
        pub id: String,
        pub title: String,
        pub project: String,
        pub work_package: String,
        pub status: SessionStatus,
        pub preview: Option<String>,
        /// Increases with every change to the session.
        pub rev: u64,
        /// Revision of the document, 1 when opened.
        pub document_revision: u32,
        pub counts: DecisionCounts,
        pub binding: Option<Binding>,
        /// The agent has said it has nothing more to add; cleared by a Send or a revision.
        pub ready: bool,
        /// Whether every active decision has a sent answer to its current version and
        /// nothing is waiting: no drafts, stale or recorded answers.
        pub all_answered: bool,
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
        pub accepted_at: Option<Timestamp>,
        /// Whose agent the session belongs to: the creator of the agent token it was opened
        /// with. None on the local daemon, where there is one person.
        #[serde(default)]
        pub owner: Option<UserRef>,
        /// People the owner invited to answer as well.
        #[serde(default)]
        pub collaborators: Vec<UserRef>,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub updated_at: Timestamp,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum DecisionStatus {
        Active,
        Withdrawn,
    }

    pub struct Decision {
        /// Assigned by the session on first appearance, never reused.
        pub number: u32,
        /// The stable id from the document.
        pub id: String,
        /// Starts at 1, increases with every revision.
        pub version: u32,
        pub status: DecisionStatus,
        pub withdrawn_reason: Option<String>,
        #[serde(flatten)]
        pub content: DecisionContent,
        pub response: Option<Response>,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub updated_at: Timestamp,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum Verdict {
        Go,
        NoGo,
    }

    /// An answer to a decision. Which parts apply depends on the decision's kind; free text
    /// is accepted on every kind.
    #[derive(Default)]
    pub struct Answer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verdict: Option<Verdict>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub choices: Vec<String>,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        pub text: String,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum ResponseState {
        /// Being filled in by the human; not yet sent.
        Draft,
        /// Committed by a Send.
        Sent,
        /// Given against an earlier version of the decision; needs confirmation.
        Stale,
        /// Captured by the agent from the thread; needs confirmation.
        RecordedFromChat,
    }

    pub struct Response {
        pub state: ResponseState,
        /// For a stale response: the state it had before the decision was revised.
        pub stale_from: Option<ResponseState>,
        /// The decision version the answer was given against.
        pub version: u32,
        pub answer: Answer,
        /// For `recorded-from-chat`: the thread message the answer came from.
        pub from_message: Option<u64>,
        /// The last answer committed by a Send, when the current one is a newer draft.
        pub last_sent: Option<SentAnswer>,
        /// Who gave this answer, where the server knows: the person's login (on a cloud host,
        /// the owner or someone they invited), or the agent token's name for one recorded
        /// from chat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub by: Option<String>,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub updated_at: Timestamp,
    }

    pub struct SentAnswer {
        pub version: u32,
        pub answer: Answer,
        /// Who gave this answer, where the server knows: the person's login (on a cloud host,
        /// the owner or someone they invited), or the agent token's name for one recorded
        /// from chat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub by: Option<String>,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub sent_at: Timestamp,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum Author {
        Human,
        Agent,
        System,
    }

    pub struct ThreadItem {
        pub seq: u64,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub at: Timestamp,
        pub author: Author,
        /// Who exactly, where the server knows: the person's login, or the agent token's
        /// name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub by: Option<String>,
        #[serde(flatten)]
        pub body: ThreadBody,
    }

    #[serde(tag = "type", rename_all = "kebab-case")]
    pub enum ThreadBody {
        Message {
            text: String,
            /// Decision numbers the message is about.
            #[serde(default)]
            about: Vec<u32>,
        },
        Event {
            event: Event,
        },
    }

    #[serde(tag = "kind", rename_all = "kebab-case")]
    pub enum Event {
        Opened {
            decisions: u32,
        },
        DocumentRevised {
            revision: u32,
            changes: Vec<Change>,
        },
        /// A Send: the human's drafts became sent answers. A turn.
        Sent {
            answers: Vec<SentEntry>,
        },
        RecordedFromChat {
            number: u32,
            version: u32,
            answer: Answer,
            from_message: Option<u64>,
        },
        AgentAttached {
            binding: Binding,
            /// The binding this one replaced.
            replaced: Option<String>,
        },
        AgentDetached {
            binding_id: String,
        },
        /// The agent has nothing more to add; the human may accept.
        Ready {
            note: Option<String>,
        },
        /// The human accepted the review. A turn.
        Accepted,
        Closed,
        /// The owner invited someone to answer.
        Invited {
            user: UserRef,
        },
        /// The owner withdrew an invitation.
        Uninvited {
            user: UserRef,
        },
    }

    pub struct SentEntry {
        pub number: u32,
        pub title: String,
        pub version: u32,
        pub answer: Answer,
        /// Who gave this answer, where the server knows: the person's login (on a cloud host,
        /// the owner or someone they invited), or the agent token's name for one recorded
        /// from chat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub by: Option<String>,
    }

    /// One change a document revision made.
    #[serde(tag = "op", rename_all = "kebab-case")]
    pub enum Change {
        Added {
            number: u32,
            id: String,
            title: String,
        },
        Revised {
            number: u32,
            id: String,
            title: String,
            version: u32,
            /// What changed: kind, title, options, recommendation, text.
            fields: Vec<String>,
            /// The decision had been withdrawn and is back.
            reinstated: bool,
        },
        Withdrawn {
            number: u32,
            id: String,
            title: String,
            reason: String,
        },
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum BindingStatus {
        Active,
        Unreachable,
        Detached,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum WakeMode {
        /// The agent long-polls with `rt review wait`.
        Pull,
    }

    /// The agent session working a review session.
    pub struct Binding {
        pub id: String,
        /// The agent harness, such as `claude-code`.
        pub harness: String,
        /// The harness's id for the agent session.
        pub session_ref: Option<String>,
        /// A link that opens the agent session, where the harness has one.
        pub url: Option<String>,
        /// A command that resumes the agent session, where the harness has one.
        pub resume: Option<String>,
        pub wake: WakeMode,
        pub status: BindingStatus,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub attached_at: Timestamp,
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
        pub last_seen_at: Option<Timestamp>,
        /// The last thread item delivered to this agent session.
        pub cursor: u64,
    }

    pub struct AttachAgent {
        pub harness: String,
        pub session_ref: Option<String>,
        pub url: Option<String>,
        pub resume: Option<String>,
    }

    pub struct OpenSession {
        /// The project; may be omitted when `repo_path` belongs to a registered project.
        pub project: Option<String>,
        /// Repository the document comes from; registers the project if it is unknown.
        pub repo_path: Option<String>,
        /// The repository's `origin` remote, which identifies the project on a cloud host.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub repo_remote: Option<String>,
        /// Overrides the document's `work_package`.
        pub work_package: Option<String>,
        pub markdown: String,
        pub agent: Option<AttachAgent>,
    }

    pub struct OpenSessionResult {
        pub state: SessionState,
        pub changes: Vec<Change>,
        pub warnings: Vec<String>,
        /// Set when the request registered the project.
        pub registered_project: Option<String>,
    }

    pub struct UpdateDocument {
        pub markdown: String,
        /// Report the changes without making them.
        #[serde(default)]
        pub dry_run: bool,
    }

    pub struct UpdateResult {
        pub changes: Vec<Change>,
        pub warnings: Vec<String>,
        pub document_revision: u32,
        pub dry_run: bool,
    }

    pub struct Document {
        pub revision: u32,
        pub markdown: String,
        pub segments: Vec<Segment>,
    }

    pub struct SessionState {
        pub session: SessionSummary,
        /// Active decisions in document order, then withdrawn ones.
        pub decisions: Vec<Decision>,
        pub thread: Vec<ThreadItem>,
        /// For a call made as a binding: the last item it had seen before this call.
        /// Items after it are new to the agent.
        pub seen_up_to: Option<u64>,
    }

    pub struct PostMessage {
        pub text: String,
        #[serde(default)]
        pub about: Vec<u32>,
        /// The agent binding posting, to update its last-seen time.
        pub binding: Option<String>,
    }

    pub struct SaveDraft {
        /// The decision version the human is answering.
        pub version: u32,
        pub answer: Answer,
    }

    pub struct RecordAnswer {
        pub answer: Answer,
        /// The thread message the answer was given in.
        pub from_message: Option<u64>,
        pub binding: Option<String>,
    }

    /// A file stored with a session: in the data directory locally, in S3 in the cloud.
    pub struct Attachment {
        /// The relative path the document links to it by.
        pub path: String,
        pub size: u64,
        pub content_type: Option<String>,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum TokenRole {
        /// Opens and works review sessions.
        Coder,
        /// Files findings (milestone 4).
        Reviewer,
    }

    /// An agent's credential on an instance. The secret is shown once, at
    /// creation; only its hash is stored.
    pub struct AgentToken {
        pub id: String,
        /// How the agent appears in logs, such as `coding agent`.
        pub name: String,
        pub role: TokenRole,
        pub projects: Vec<String>,
        pub creator: UserRef,
        #[cfg_attr(feature = "openapi", schema(value_type = String, format = DateTime))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
        pub expires_at: Option<Timestamp>,
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
        pub last_used_at: Option<Timestamp>,
        #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
        pub revoked_at: Option<Timestamp>,
        pub revoked_reason: Option<String>,
    }

    pub struct CreateToken {
        pub name: String,
        pub role: TokenRole,
        pub projects: Vec<String>,
        /// Days until it expires; never when omitted.
        pub expires_in_days: Option<u32>,
    }

    pub struct CreatedToken {
        /// The secret, shown this once.
        pub token: String,
        pub record: AgentToken,
    }

    pub struct Invite {
        /// The person's login on the code host.
        pub login: String,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum PrincipalKind {
        /// The local daemon's one person, or an agent acting for them.
        Local,
        /// A person signed in on a cloud host.
        User,
        /// An agent with a token on a cloud host.
        Agent,
    }

    /// Who the server takes the caller for.
    pub struct Whoami {
        pub kind: PrincipalKind,
        pub user: Option<UserRef>,
        pub token: Option<AgentToken>,
    }

    /// `rt init` exchanges the one-time code the browser brought back for an agent token
    /// (RFC 8252: a loopback redirect, with PKCE).
    pub struct ConnectExchange {
        pub code: String,
        /// The PKCE verifier whose S256 challenge started the flow.
        pub code_verifier: String,
        /// The loopback address the code was sent to.
        pub redirect_uri: String,
    }

    pub struct Connected {
        /// The new agent token's secret, shown this once.
        pub token: String,
        pub record: AgentToken,
        /// The project the repository is.
        pub project: String,
    }

    pub struct UploadRequest {
        pub size: u64,
        pub content_type: String,
    }

    /// Where to send an attachment's bytes. With a `url`, PUT them there with `headers`,
    /// then complete the upload; without one, PUT them to the attachment's API path.
    pub struct UploadTicket {
        pub url: Option<String>,
        #[serde(default)]
        pub headers: Vec<UploadHeader>,
    }

    pub struct UploadHeader {
        pub name: String,
        pub value: String,
    }

    pub struct MarkReady {
        /// What the human is accepting, in a line.
        pub note: Option<String>,
        pub binding: Option<String>,
    }

    pub struct SendResult {
        /// Decision numbers whose drafts were sent.
        pub sent: Vec<u32>,
        pub seq: u64,
    }

    #[serde(rename_all = "kebab-case")]
    #[derive(Copy, Eq)]
    pub enum WaitOutcome {
        /// A human turn arrived: a chat message or a Send.
        Turn,
        Timeout,
        /// This binding was detached or replaced; the agent session should stop.
        Detached,
        Closed,
    }

    pub struct WaitResult {
        pub outcome: WaitOutcome,
        /// Thread items new to the binding, human and agent, events included.
        pub new_items: Vec<ThreadItem>,
        pub state: SessionState,
        /// Set by a server that does not hold waits open (the cloud host): on a timeout,
        /// ask again after this many seconds until your own deadline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub poll_after_secs: Option<u32>,
    }

    pub struct Changes {
        pub rev: u64,
        /// Whether an agent is waiting for the human on the session right now.
        pub agent_waiting: bool,
        /// Set by a server that answers at once (the cloud host): ask again after this
        /// many seconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub poll_after_secs: Option<u32>,
    }

    pub struct Handoff {
        pub text: String,
    }

    pub struct CloseSession {
        /// Close even if some active decisions have no sent answer.
        #[serde(default)]
        pub force: bool,
    }

    pub struct Record {
        /// The Markdown record of the session.
        pub markdown: String,
    }
}

impl Answer {
    pub fn is_empty(&self) -> bool {
        self.verdict.is_none() && self.choices.is_empty() && self.text.trim().is_empty()
    }

    /// A one-line rendering for listings and the thread.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        match self.verdict {
            Some(Verdict::Go) => parts.push("go".to_owned()),
            Some(Verdict::NoGo) => parts.push("no-go".to_owned()),
            None => {}
        }
        if !self.choices.is_empty() {
            parts.push(self.choices.join("; "));
        }
        let text = self.text.trim();
        if !text.is_empty() {
            let one_line = text.lines().collect::<Vec<_>>().join(" ");
            parts.push(format!("\"{one_line}\""));
        }
        if parts.is_empty() {
            "(empty)".to_owned()
        } else {
            parts.join(" — ")
        }
    }

    /// Checks the answer fits the decision.
    pub fn validate(&self, content: &DecisionContent) -> Result<(), String> {
        match content.kind {
            DecisionKind::GoNogo => {
                if !self.choices.is_empty() {
                    return Err("a go/no-go decision takes a verdict, not choices".into());
                }
            }
            DecisionKind::Choice => {
                if self.verdict.is_some() {
                    return Err("a choice takes choices, not a verdict".into());
                }
                if !content.multi && self.choices.len() > 1 {
                    return Err("this choice takes one option".into());
                }
                if let Some(bad) = self.choices.iter().find(|c| !content.options.contains(c)) {
                    return Err(format!("`{bad}` is not one of the options"));
                }
            }
            DecisionKind::Text => {
                if self.verdict.is_some() || !self.choices.is_empty() {
                    return Err("a text decision takes text only".into());
                }
            }
        }
        Ok(())
    }
}

impl ThreadItem {
    /// Whether this item is a human turn: a chat message or a Send.
    pub fn is_turn(&self) -> bool {
        match &self.body {
            ThreadBody::Message { .. } => self.author == Author::Human,
            ThreadBody::Event { event } => matches!(event, Event::Sent { .. } | Event::Accepted),
        }
    }
}

impl std::fmt::Display for DecisionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DecisionKind::GoNogo => "go-nogo",
            DecisionKind::Choice => "choice",
            DecisionKind::Text => "text",
        })
    }
}

impl std::fmt::Display for WorkPackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WorkPackageStatus::Planning => "planning",
            WorkPackageStatus::Building => "building",
            WorkPackageStatus::Reviewing => "reviewing",
            WorkPackageStatus::Accepted => "accepted",
            WorkPackageStatus::Integrated => "integrated",
            WorkPackageStatus::Deployed => "deployed",
        })
    }
}

impl std::str::FromStr for WorkPackageStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        serde_json::from_value(serde_json::Value::String(s.to_owned())).map_err(|_| {
            format!("unknown status `{s}`: planning, building, reviewing, accepted, integrated or deployed")
        })
    }
}

impl std::fmt::Display for ResponseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ResponseState::Draft => "draft",
            ResponseState::Sent => "sent",
            ResponseState::Stale => "stale",
            ResponseState::RecordedFromChat => "recorded-from-chat",
        })
    }
}
