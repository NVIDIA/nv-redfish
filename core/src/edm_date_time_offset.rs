// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use core::str::FromStr;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fmt::Error as FmtError;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Type corresponding to `Edm.DateTimeOffset`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdmDateTimeOffset(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);

impl From<OffsetDateTime> for EdmDateTimeOffset {
    fn from(dt: OffsetDateTime) -> Self {
        Self(dt)
    }
}

impl From<EdmDateTimeOffset> for OffsetDateTime {
    fn from(w: EdmDateTimeOffset) -> Self {
        w.0
    }
}

impl Display for EdmDateTimeOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let s = self.0.format(&Rfc3339).map_err(|_| FmtError)?;
        f.write_str(&s)
    }
}

impl FromStr for EdmDateTimeOffset {
    type Err = time::error::Parse;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dt = OffsetDateTime::parse(s, &Rfc3339)?;
        Ok(Self(dt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::UtcOffset;

    #[test]
    fn parses_and_displays_utc_z() {
        let s = "2021-03-04T05:06:07Z";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!(w.to_string(), s);

        let dt: OffsetDateTime = w.into();
        assert_eq!(dt.offset(), UtcOffset::UTC);
    }

    #[test]
    fn parses_utc_plus00_canonicalizes_to_z_on_display() {
        let s = "2021-03-04T05:06:07+00:00";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        let displayed = w.to_string();
        assert!(displayed.ends_with('Z'));
    }

    #[test]
    fn parses_and_displays_positive_offset() {
        let s = "2021-03-04T10:36:07+05:30"; // same instant as 05:06:07Z
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!(w.to_string(), s);

        let dt: OffsetDateTime = w.into();
        assert_eq!(dt.offset(), UtcOffset::from_hms(5, 30, 0).unwrap());
    }

    #[test]
    fn parses_and_displays_fractional_seconds() {
        let s = "2021-03-04T05:06:07.123456789Z";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!(w.to_string(), s);
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert!("not-a-date".parse::<EdmDateTimeOffset>().is_err());
        // RFC3339 requires an explicit offset
        assert!("2021-03-04T05:06:07".parse::<EdmDateTimeOffset>().is_err());
    }

    #[test]
    fn serde_serializes_conformant_strings() {
        // UTC Z
        let w_z: EdmDateTimeOffset = "2021-03-04T05:06:07Z".parse().unwrap();
        let json_z = serde_json::to_string(&w_z).unwrap();
        assert_eq!(json_z, r#""2021-03-04T05:06:07Z""#);

        // Non-UTC offset preserved
        let w_pos: EdmDateTimeOffset = "2021-03-04T10:36:07+05:30".parse().unwrap();
        let json_pos = serde_json::to_string(&w_pos).unwrap();
        assert_eq!(json_pos, r#""2021-03-04T10:36:07+05:30""#);

        // Fractional seconds retained
        let w_frac: EdmDateTimeOffset = "2021-03-04T05:06:07.123456789Z".parse().unwrap();
        let json_frac = serde_json::to_string(&w_frac).unwrap();
        assert_eq!(json_frac, r#""2021-03-04T05:06:07.123456789Z""#);

        // Canonicalize +00:00 to Z
        let w_plus00: EdmDateTimeOffset = "2021-03-04T05:06:07+00:00".parse().unwrap();
        let json_plus00 = serde_json::to_string(&w_plus00).unwrap();
        assert_eq!(json_plus00, r#""2021-03-04T05:06:07Z""#);
    }

    #[test]
    fn serde_deserializes_from_conformant_strings() {
        // UTC Z
        let s_z = r#""2021-03-04T05:06:07Z""#;
        let w_z: EdmDateTimeOffset = serde_json::from_str(s_z).unwrap();
        assert_eq!(w_z.to_string(), "2021-03-04T05:06:07Z");
        let dt_z: OffsetDateTime = w_z.into();
        assert_eq!(dt_z.offset(), UtcOffset::UTC);

        // Non-UTC offset preserved
        let s_pos = r#""2021-03-04T10:36:07+05:30""#;
        let w_pos: EdmDateTimeOffset = serde_json::from_str(s_pos).unwrap();
        assert_eq!(w_pos.to_string(), "2021-03-04T10:36:07+05:30");
        let dt_pos: OffsetDateTime = w_pos.into();
        assert_eq!(dt_pos.offset(), UtcOffset::from_hms(5, 30, 0).unwrap());

        // Fractional seconds retained
        let s_frac = r#""2021-03-04T05:06:07.123456789Z""#;
        let w_frac: EdmDateTimeOffset = serde_json::from_str(s_frac).unwrap();
        assert_eq!(w_frac.to_string(), "2021-03-04T05:06:07.123456789Z");
    }

    #[test]
    fn parses_and_displays_negative_offset() {
        let s = "2021-03-04T00:06:07-05:00";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!(w.to_string(), s);

        let dt: OffsetDateTime = w.into();
        assert_eq!(dt.offset(), UtcOffset::from_hms(-5, 0, 0).unwrap());
    }

    #[test]
    fn parses_fractional_with_non_utc_offset() {
        let s = "2021-03-04T05:06:07.5+01:00";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!(w.to_string(), s);
    }

    #[test]
    fn parses_boundary_offsets() {
        // Commonly used extrema
        let s_plus = "2021-03-04T12:00:00+14:00";
        let w_plus: EdmDateTimeOffset = s_plus.parse().unwrap();
        assert_eq!(w_plus.to_string(), s_plus);

        let s_minus = "2021-03-04T12:00:00-12:00";
        let w_minus: EdmDateTimeOffset = s_minus.parse().unwrap();
        assert_eq!(w_minus.to_string(), s_minus);
    }

    #[test]
    fn rejects_leap_second() {
        assert!("2021-03-04T23:59:60Z".parse::<EdmDateTimeOffset>().is_err());
    }

    #[test]
    fn canonicalizes_negative_zero_offset_to_z() {
        let s = "2021-03-04T05:06:07-00:00";
        let w: EdmDateTimeOffset = s.parse().unwrap();
        assert_eq!("2021-03-04T05:06:07Z", w.to_string());
    }
}
