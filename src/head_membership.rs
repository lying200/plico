//! Retain only displayed OIDs while draining one immutable HEAD traversal.
use crate::git::{Cancel, is_oid};
use std::{collections::HashSet, io::{self, BufRead, BufReader, Read}};
pub(crate) fn read(input: impl Read, mut remaining: HashSet<String>, token: &Cancel) -> io::Result<HashSet<String>> {
    let mut input=BufReader::new(input);
    let mut included=HashSet::new();
    let mut line=Vec::with_capacity(66);
    loop {
        if token.cancelled() {return Err(io::Error::other("Cancelled"));}
        line.clear();
        // SHA-256 + CRLF is the longest valid record. Never allocate for an
        // unbounded line, even when a Git wrapper produces unexpected output.
        if input.by_ref().take(66).read_until(b'\n',&mut line)?==0 {break;}
        let raw=line.strip_suffix(b"\n").unwrap_or(&line);
        let raw=raw.strip_suffix(b"\r").unwrap_or(raw);
        let oid=std::str::from_utf8(raw).map_err(io::Error::other)?;
        if !is_oid(oid) {return Err(io::Error::other("Invalid HEAD traversal record"));}
        if remaining.remove(oid) {included.insert(oid.to_owned());}
    }
    Ok(included)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_only_window_ids_across_many_records_and_rejects_invalid_or_cancelled_data() {
        let ids=(0..2500).map(|n|format!("{n:040x}")).collect::<Vec<_>>();
        let outside=format!("{:040x}",3000);
        let requested=HashSet::from([ids[0].clone(),ids[2300].clone(),outside.clone()]);
        let got=read(ids.join("\n").as_bytes(),requested,&Cancel::default()).unwrap();
        assert_eq!(got,HashSet::from([ids[0].clone(),ids[2300].clone()]));
        assert!(read("x".repeat(10000).as_bytes(),HashSet::new(),&Cancel::default()).is_err());
        let token=Cancel::default();token.cancel();
        assert!(read(&b""[..],HashSet::new(),&token).is_err());
        let sha256="a".repeat(64);
        assert!(read(format!("{sha256}\r\n").as_bytes(),HashSet::from([sha256.clone()]),&Cancel::default()).unwrap().contains(&sha256));
    }
}
