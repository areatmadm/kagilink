//! Receiver-side selection of a sender among nearby advertisers (docs/BLE_GATT_SPEC.md section 3,
//! D026). Pure logic, no I/O, no unsafe. The receiver NEVER auto-connects: nothing here returns a
//! default choice, the caller must pass the index the USER picked.

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
#[derive(Clone, PartialEq, Eq)]
pub struct SelectionList {
    entries: Vec<Candidate>,
    /// Opaque per-row connection key, same order as `entries`; never shown, printed or stored.
    sources: Vec<u64>,
}

impl core::fmt::Debug for SelectionList {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SelectionList")
            .field("entries", &self.entries)
            .finish()
    }
}

/// An opaque handle to the advertiser the user chose. Its content is never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectTarget(u64);

impl ConnectTarget {
    /// The platform key (a BLE address as a number) for the connect call.
    pub fn key(&self) -> u64 {
        self.0
    }
}

impl core::fmt::Debug for ConnectTarget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ConnectTarget(..)")
    }
}

/// True if `s` is exactly `TAG_LEN` characters, all from `TAG_ALPHABET` (uppercase, case-sensitive).
pub fn is_valid_tag(s: &str) -> bool {
    s.chars().count() == TAG_LEN && s.chars().all(|c| TAG_ALPHABET.contains(c))
}

/// The tag from a local name of the exact form `KL-XXXX`; `None` unless valid.
pub fn tag_from_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix(NAME_PREFIX)?;
    is_valid_tag(rest).then(|| rest.to_owned())
}

/// The tag from service data of exactly `TAG_LEN` ASCII bytes; `None` unless valid.
pub fn tag_from_service_data(data: &[u8]) -> Option<String> {
    if data.len() != TAG_LEN || !data.is_ascii() {
        return None;
    }
    let s = std::str::from_utf8(data).ok()?;
    is_valid_tag(s).then(|| s.to_owned())
}

struct Merged {
    source: u64,
    name: Option<String>,
    data: Option<Vec<u8>>,
    rssi: Option<i16>,
}

fn signal_bar(rssi: Option<i16>) -> SignalBar {
    match rssi {
        None => SignalBar::Unknown,
        Some(r) if r >= -60 => SignalBar::Strong,
        Some(r) if r >= -75 => SignalBar::Medium,
        Some(_) => SignalBar::Weak,
    }
}

fn read_tag(m: &Merged) -> (Option<String>, Option<Unselectable>) {
    let from_name = m.name.as_deref().map(tag_from_name);
    let from_data = m.data.as_deref().map(tag_from_service_data);
    match (from_name, from_data) {
        (None, None) => (None, Some(Unselectable::NoTag)),
        (Some(Some(t)), None) | (None, Some(Some(t))) => (Some(t), None),
        (Some(Some(a)), Some(Some(b))) if a == b => (Some(a), None),
        _ => (None, Some(Unselectable::TagConflict)),
    }
}

/// Build the list from raw observations (merging packets of one advertiser, validating tags,
/// flagging duplicates, sorting by signal strength).
pub fn build_list(raws: &[RawAdvertisement]) -> SelectionList {
    let mut merged: Vec<Merged> = Vec::new();
    for r in raws {
        let idx = match merged.iter().position(|m| m.source == r.source) {
            Some(i) => i,
            None => {
                merged.push(Merged {
                    source: r.source,
                    name: None,
                    data: None,
                    rssi: None,
                });
                merged.len() - 1
            }
        };
        let m = &mut merged[idx];
        if r.local_name.is_some() {
            m.name.clone_from(&r.local_name);
        }
        if r.service_data.is_some() {
            m.data.clone_from(&r.service_data);
        }
        if let Some(v) = r.rssi_dbm {
            m.rssi = Some(m.rssi.map_or(v, |old| old.max(v)));
        }
    }

    let mut entries: Vec<Candidate> = merged
        .iter()
        .map(|m| {
            let (tag, blocked) = read_tag(m);
            Candidate {
                tag,
                signal: signal_bar(m.rssi),
                rssi_dbm: m.rssi,
                blocked,
            }
        })
        .collect();
    let mut sources: Vec<u64> = merged.iter().map(|m| m.source).collect();

    // Every entry showing a tag that another (different) source also shows is blocked.
    let tags: Vec<Option<String>> = entries.iter().map(|e| e.tag.clone()).collect();
    for (i, e) in entries.iter_mut().enumerate() {
        if let Some(t) = &tags[i] {
            let dup = tags
                .iter()
                .enumerate()
                .any(|(j, o)| j != i && o.as_ref() == Some(t));
            if dup {
                e.blocked = Some(Unselectable::DuplicateTag);
            }
        }
    }

    // Stable sort: strongest first, unknown RSSI last. Sources move together with their rows.
    let mut paired: Vec<(Candidate, u64)> = entries.drain(..).zip(sources.drain(..)).collect();
    paired.sort_by(|(a, _), (b, _)| match (a.rssi_dbm, b.rssi_dbm) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    for (c, src) in paired {
        entries.push(c);
        sources.push(src);
    }
    SelectionList { entries, sources }
}

impl SelectionList {
    /// All rows, strongest signal first.
    pub fn entries(&self) -> &[Candidate] {
        &self.entries
    }

    /// The row the user picked. Never called with a default index by the program itself.
    pub fn choose(&self, index: usize) -> Result<&Candidate, SelectionError> {
        let c = self.entries.get(index).ok_or(SelectionError::OutOfRange)?;
        match &c.blocked {
            Some(reason) => Err(SelectionError::NotSelectable(reason.clone())),
            None => Ok(c),
        }
    }

    /// Map what the user typed to a row index (0-based): a row number (`2`) or a tag (`KL-AB23` or `AB23`).
    /// This only translates the user's explicit input; it never picks a row by itself, and a tag that is
    /// shown twice (duplicate) is not mapped because `choose` refuses it anyway.
    pub fn index_for_choice(&self, input: &str) -> Option<usize> {
        let t = input.trim();
        if let Ok(n) = t.parse::<usize>() {
            return n.checked_sub(1);
        }
        let tag = t.strip_prefix("KL-").unwrap_or(t).to_ascii_uppercase();
        if !is_valid_tag(&tag) {
            return None;
        }
        let mut found = None;
        for (i, c) in self.entries.iter().enumerate() {
            if c.tag.as_deref() == Some(tag.as_str()) {
                if found.is_some() {
                    return None; // two rows with the same tag: never guess
                }
                found = Some(i);
            }
        }
        found
    }

    /// Like `choose`, but returns the opaque connect handle. Same refusals, same explicit index.
    pub fn choose_target(&self, index: usize) -> Result<ConnectTarget, SelectionError> {
        self.choose(index)?;
        Ok(ConnectTarget(self.sources[index]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(
        source: u64,
        name: Option<&str>,
        data: Option<&[u8]>,
        rssi: Option<i16>,
    ) -> RawAdvertisement {
        RawAdvertisement {
            local_name: name.map(str::to_owned),
            service_data: data.map(<[u8]>::to_vec),
            rssi_dbm: rssi,
            source,
        }
    }

    #[test]
    fn target_follows_the_sorted_row_and_is_redacted() {
        let list = build_list(&[
            raw(111, Some("KL-AB23"), None, Some(-90)),
            raw(222, Some("KL-CD45"), None, Some(-50)),
        ]);
        assert_eq!(list.choose_target(0).unwrap().key(), 222);
        assert_eq!(list.choose_target(1).unwrap().key(), 111);
        assert_eq!(
            format!("{:?}", list.choose_target(0).unwrap()),
            "ConnectTarget(..)"
        );
        assert!(list.choose_target(2).is_err());
        let dup = build_list(&[
            raw(1, Some("KL-AB23"), None, Some(-50)),
            raw(2, Some("KL-AB23"), None, Some(-60)),
        ]);
        assert!(dup.choose_target(0).is_err());
    }

    #[test]
    fn typed_choice_maps_row_numbers_and_tags() {
        let list = build_list(&[
            raw(1, Some("KL-AB23"), None, Some(-50)),
            raw(2, Some("KL-CD45"), None, Some(-60)),
        ]);
        assert_eq!(list.index_for_choice("1"), Some(0));
        assert_eq!(list.index_for_choice(" 2 "), Some(1));
        assert_eq!(list.index_for_choice("0"), None);
        assert_eq!(list.index_for_choice("KL-CD45"), Some(1));
        assert_eq!(list.index_for_choice("cd45"), Some(1));
        assert_eq!(list.index_for_choice("ZZZZ"), None);
        assert_eq!(list.index_for_choice("KL-CD4"), None);
        let dup = build_list(&[
            raw(1, Some("KL-AB23"), None, Some(-50)),
            raw(2, Some("KL-AB23"), None, Some(-60)),
        ]);
        assert_eq!(dup.index_for_choice("AB23"), None);
    }

    #[test]
    fn tag_validation() {
        assert!(is_valid_tag("AB23"));
        for bad in [
            "ab23",
            "AB2",
            "AB234",
            "AB2O",
            "AB21",
            "AB2I",
            "AB 3",
            "AB2\u{00C9}",
            "",
            " AB23",
        ] {
            assert!(!is_valid_tag(bad), "{bad:?}");
        }
        assert_eq!(tag_from_name("KL-AB23"), Some("AB23".into()));
        for bad in [
            "KL-abcd", "XX-ABCD", "KL-ABC", "KL-ABCDE", "kl-ABCD", "KL-ABCD ", " KL-ABCD", "ABCD",
        ] {
            assert_eq!(tag_from_name(bad), None, "{bad:?}");
        }
        assert_eq!(tag_from_service_data(b"AB23"), Some("AB23".into()));
        assert_eq!(tag_from_service_data(b"AB2"), None);
        assert_eq!(tag_from_service_data(b"AB234"), None);
        assert_eq!(tag_from_service_data(b"ab23"), None);
        assert_eq!(tag_from_service_data(&[0x41, 0x42, 0x32, 0xC3]), None);
    }

    #[test]
    fn empty_input() {
        let l = build_list(&[]);
        assert!(l.entries().is_empty());
        assert_eq!(l.choose(0), Err(SelectionError::OutOfRange));
    }

    #[test]
    fn name_only_data_only_and_agree() {
        let l = build_list(&[
            raw(1, Some("KL-AB23"), None, Some(-50)),
            raw(2, None, Some(b"CD45"), Some(-51)),
            raw(3, Some("KL-EF67"), Some(b"EF67"), Some(-52)),
        ]);
        let tags: Vec<_> = l.entries().iter().map(|e| e.tag.clone()).collect();
        assert_eq!(
            tags,
            vec![
                Some("AB23".into()),
                Some("CD45".into()),
                Some("EF67".into())
            ]
        );
        assert!(l.entries().iter().all(|e| e.blocked.is_none()));
    }

    #[test]
    fn disagreement_and_malformed_are_conflict() {
        let l = build_list(&[
            raw(1, Some("KL-AB23"), Some(b"CD45"), Some(-40)),
            raw(2, Some("KL-abcd"), None, Some(-41)),
            raw(3, Some("XX-ABCD"), None, Some(-42)),
            raw(4, None, Some(b"ABC"), Some(-43)),
            raw(5, Some("KL-AB23"), Some(b"abcd"), Some(-44)),
        ]);
        assert_eq!(l.entries().len(), 5);
        for e in l.entries() {
            assert_eq!(e.tag, None);
            assert_eq!(e.blocked, Some(Unselectable::TagConflict));
        }
    }

    #[test]
    fn no_tag() {
        let l = build_list(&[raw(1, None, None, Some(-40))]);
        assert_eq!(l.entries()[0].blocked, Some(Unselectable::NoTag));
        assert_eq!(l.entries()[0].tag, None);
        assert_eq!(
            l.choose(0),
            Err(SelectionError::NotSelectable(Unselectable::NoTag))
        );
    }

    #[test]
    fn rssi_boundaries() {
        let cases = [
            (-60, SignalBar::Strong),
            (-61, SignalBar::Medium),
            (-75, SignalBar::Medium),
            (-76, SignalBar::Weak),
            (-30, SignalBar::Strong),
            (-100, SignalBar::Weak),
        ];
        for (r, bar) in cases {
            let l = build_list(&[raw(1, Some("KL-AB23"), None, Some(r))]);
            assert_eq!(l.entries()[0].signal, bar, "rssi {r}");
        }
        let l = build_list(&[raw(1, Some("KL-AB23"), None, None)]);
        assert_eq!(l.entries()[0].signal, SignalBar::Unknown);
    }

    #[test]
    fn same_source_merges() {
        let l = build_list(&[
            raw(7, Some("KL-AB23"), None, Some(-80)),
            raw(7, None, Some(b"AB23"), Some(-55)),
            raw(7, None, None, Some(-70)),
            raw(7, None, None, None),
        ]);
        assert_eq!(l.entries().len(), 1);
        let e = &l.entries()[0];
        assert_eq!(e.tag.as_deref(), Some("AB23"));
        assert_eq!(e.rssi_dbm, Some(-55));
        assert_eq!(e.signal, SignalBar::Strong);
        assert_eq!(e.blocked, None);
    }

    #[test]
    fn same_source_keeps_last_values() {
        let l = build_list(&[
            raw(7, Some("KL-AB23"), None, None),
            raw(7, Some("KL-CD45"), None, None),
        ]);
        assert_eq!(l.entries()[0].tag.as_deref(), Some("CD45"));
    }

    #[test]
    fn duplicate_tags_blocked_and_refused() {
        let l = build_list(&[
            raw(1, Some("KL-AB23"), None, Some(-50)),
            raw(2, None, Some(b"AB23"), Some(-60)),
            raw(3, Some("KL-CD45"), None, Some(-70)),
        ]);
        assert_eq!(l.entries().len(), 3);
        for i in 0..2 {
            assert_eq!(l.entries()[i].tag.as_deref(), Some("AB23"));
            assert_eq!(l.entries()[i].blocked, Some(Unselectable::DuplicateTag));
            assert_eq!(
                l.choose(i),
                Err(SelectionError::NotSelectable(Unselectable::DuplicateTag))
            );
        }
        assert!(l.choose(2).is_ok());
    }

    #[test]
    fn single_valid_entry_needs_explicit_choose() {
        let l = build_list(&[raw(1, Some("KL-AB23"), None, Some(-50))]);
        assert_eq!(l.entries().len(), 1);
        assert_eq!(l.choose(0).unwrap().tag.as_deref(), Some("AB23"));
        assert_eq!(l.choose(1), Err(SelectionError::OutOfRange));
    }

    #[test]
    fn sorting_none_last_and_stable() {
        let l = build_list(&[
            raw(1, Some("KL-AAAA"), None, None),
            raw(2, Some("KL-BBBB"), None, Some(-70)),
            raw(3, Some("KL-CCCC"), None, Some(-50)),
            raw(4, Some("KL-DDDD"), None, Some(-70)),
            raw(5, Some("KL-EEEE"), None, None),
        ]);
        let order: Vec<_> = l.entries().iter().map(|e| e.tag.clone().unwrap()).collect();
        assert_eq!(order, ["CCCC", "BBBB", "DDDD", "AAAA", "EEEE"]);
    }

    #[test]
    fn source_never_exposed() {
        let src = 7_654_321_987_u64;
        let l = build_list(&[raw(src, Some("KL-AB23"), Some(b"AB23"), Some(-50))]);
        let dump = format!("{l:?}");
        assert!(!dump.contains(&src.to_string()));
        assert!(!dump.contains("source"));
    }
}
