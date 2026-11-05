//! Receiver-side selection of a sender among nearby advertisers (docs/BLE_GATT_SPEC.md section 3,
//! D026). Pure logic, no I/O, no unsafe. The receiver NEVER auto-connects: nothing here returns a
//! default choice, the caller must pass the index the USER picked.
//!
//! STUB: the types below are the agreed interface; the bodies are filled in by the selection task.

/// The 31-symbol tag alphabet (no look-alike characters).
pub const TAG_ALPHABET: &str = "23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// Tag length in characters.
pub const TAG_LEN: usize = 4;
/// Prefix of the local-name form of the tag: `KL-XXXX`.
pub const NAME_PREFIX: &str = "KL-";

/// What a scan saw for one advertiser, already merged by the scanner per advertiser where it can.
/// `source` is an opaque per-advertiser key (for example the BLE address as a number). It is used
/// only to group packets, must never be displayed or stored, and carries no meaning across scans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawAdvertisement {
    /// Complete local name from the advertisement data, if any.
    pub local_name: Option<String>,
    /// Service data bytes for the KL-V1 service UUID (without the UUID itself), if any.
    pub service_data: Option<Vec<u8>>,
    /// Received signal strength in dBm, if reported.
    pub rssi_dbm: Option<i16>,
    /// Opaque grouping key, never shown.
    pub source: u64,
}

/// Coarse signal bar shown next to a tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignalBar {
    /// No RSSI reported.
    Unknown,
    /// Weak.
    Weak,
    /// Medium.
    Medium,
    /// Strong.
    Strong,
}

/// Why an entry cannot be selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unselectable {
    /// No valid tag in either form.
    NoTag,
    /// Both forms present but they disagree, or one form is malformed.
    TagConflict,
    /// Another advertiser in the list shows the same tag.
    DuplicateTag,
}

/// One row of the list shown to the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The valid tag, if there is exactly one trustworthy reading of it.
    pub tag: Option<String>,
    /// Coarse signal bar.
    pub signal: SignalBar,
    /// Raw RSSI, for sorting and diagnostics only.
    pub rssi_dbm: Option<i16>,
    /// `None` if the user may choose this row, otherwise the reason it cannot be chosen.
    pub blocked: Option<Unselectable>,
}

/// Why a choice was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// The index is not in the list.
    OutOfRange,
    /// The row exists but cannot be chosen.
    NotSelectable(Unselectable),
}

/// The list shown to the user, strongest signal first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionList {
    entries: Vec<Candidate>,
}

/// Build the list from raw observations (merging packets of one advertiser, validating tags,
/// flagging duplicates, sorting by signal strength).
pub fn build_list(_raws: &[RawAdvertisement]) -> SelectionList {
    SelectionList {
        entries: Vec::new(),
    }
}

impl SelectionList {
    /// All rows, strongest signal first.
    pub fn entries(&self) -> &[Candidate] {
        &self.entries
    }

    /// The row the user picked. Never called with a default index by the program itself.
    pub fn choose(&self, _index: usize) -> Result<&Candidate, SelectionError> {
        Err(SelectionError::OutOfRange)
    }
}
