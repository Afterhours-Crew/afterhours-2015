// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Bounded launcher framing, XML, typed envelopes and transform codecs.
//! Session policy, listener and authentication decisions are separate.
pub mod cipher;
pub mod frame;
pub mod message;
pub mod xml;
