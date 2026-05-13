//! Envelope codec for `CommitLLM` receipts.
//!
//! Implements `VIKY` (verifier-key), `VIRC` (chat receipt), and `VIAU` (audit
//! receipt) headers per RFC-0003. Leaf crate: no async, no networking.

#![cfg_attr(not(test), deny(unsafe_code))]

pub fn placeholder() {}
