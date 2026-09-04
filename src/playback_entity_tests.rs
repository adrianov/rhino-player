// Playback-entity behavior specs: path → entity resolution, shared DVD title keys,
// global resume mapping, and per-chapter stream identity. Fixtures build real temp DVD trees.

use super::*;
use std::fs;

include!("playback_entity_tests_fixtures.rs");
include!("playback_entity_tests_dvd.rs");
include!("playback_entity_tests_resume.rs");
