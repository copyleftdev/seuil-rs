//! Injectable environment for deterministic simulation testing.
//!
//! All sources of non-determinism (time, randomness, UUIDs) are abstracted behind
//! the [`Environment`] trait. Production code uses [`RealEnvironment`]. Test code
//! uses [`MockEnvironment`] with a seed for fully reproducible evaluation.

use std::cell::{Cell, RefCell};

use chrono::DateTime;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use uuid::Uuid;

/// Get current time in milliseconds since Unix epoch (works on all platforms including WASM).
fn platform_now_millis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_millis() as u64
    }
}

/// Get current time as ISO 8601 string (works on all platforms including WASM).
fn platform_now_iso() -> String {
    let millis = platform_now_millis() as i64;
    let secs = millis / 1000;
    let nsecs = ((millis % 1000) * 1_000_000) as u32;
    if let Some(dt) = DateTime::from_timestamp(secs, nsecs) {
        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    } else {
        // Fallback — should never happen
        "1970-01-01T00:00:00.000Z".to_string()
    }
}

/// Abstraction over all non-deterministic operations.
///
/// Every function that touches time, randomness, or UUIDs goes through this trait.
/// This enables deterministic simulation testing (DST) by injecting a [`MockEnvironment`]
/// with a fixed seed, making all evaluation fully reproducible.
pub trait Environment {
    /// Current time as ISO 8601 string (used by `$now()`).
    fn now_iso(&self) -> String;

    /// Current time as milliseconds since Unix epoch (used by `$millis()`).
    fn now_millis(&self) -> u64;

    /// Random f64 in `[0.0, 1.0)` (used by `$random()`).
    fn random_f64(&self) -> f64;

    /// Random UUID v4 string (used by `$uuid()`).
    fn random_uuid(&self) -> String;

    /// Monotonic elapsed time in milliseconds since `since` timestamp.
    /// Used for evaluation timeout checking.
    fn elapsed_millis(&self, since: u64) -> u64;

    /// Returns a timestamp marking "now" for use as the `since` argument
    /// to [`Environment::elapsed_millis`]. In production this is a monotonic instant;
    /// in mock mode it's the current simulated clock value.
    fn timestamp(&self) -> u64;
}

// ---------------------------------------------------------------------------
// RealEnvironment — production use
// ---------------------------------------------------------------------------

/// Production environment using real system time and randomness.
pub struct RealEnvironment {
    _private: (),
}

impl RealEnvironment {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Const constructor for use in statics.
    pub const fn new_const() -> Self {
        Self { _private: () }
    }
}

impl Default for RealEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment for RealEnvironment {
    fn now_iso(&self) -> String {
        platform_now_iso()
    }

    fn now_millis(&self) -> u64 {
        platform_now_millis()
    }

    fn random_f64(&self) -> f64 {
        rand::rng().random::<f64>()
    }

    fn random_uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn elapsed_millis(&self, since: u64) -> u64 {
        let now = self.now_millis();
        now.saturating_sub(since)
    }

    fn timestamp(&self) -> u64 {
        self.now_millis()
    }
}

// ---------------------------------------------------------------------------
// MockEnvironment — deterministic simulation testing
// ---------------------------------------------------------------------------

/// Fully deterministic environment for testing.
///
/// All operations are derived from a seed, making evaluation 100% reproducible.
/// The clock only advances when explicitly told to, enabling time compression.
///
/// # Example
/// ```
/// use seuil::clock::{MockEnvironment, Environment};
///
/// let env = MockEnvironment::new(0xDEAD_BEEF);
/// assert_eq!(env.now_millis(), 1_000_000_000_000); // fixed epoch
/// // RNG advances on each call — NOT equal:
/// let r1 = env.random_f64();
/// let r2 = env.random_f64();
/// assert_ne!(r1, r2);
/// ```
pub struct MockEnvironment {
    seed: u64,
    clock_millis: Cell<u64>,
    rng: RefCell<StdRng>,
}

impl MockEnvironment {
    /// Create a new mock environment with the given seed.
    ///
    /// The simulated clock starts at Unix timestamp 1_000_000_000_000 ms
    /// (2001-09-09T01:46:40Z) to avoid edge cases around epoch zero.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            clock_millis: Cell::new(1_000_000_000_000), // 2001-09-09T01:46:40Z
            rng: RefCell::new(StdRng::seed_from_u64(seed)),
        }
    }

    /// Get the seed used to create this environment.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Advance the simulated clock by `delta_ms` milliseconds.
    pub fn advance_clock(&self, delta_ms: u64) {
        let now = self.clock_millis.get();
        self.clock_millis.set(now.saturating_add(delta_ms));
    }

    /// Set the simulated clock to an exact value.
    pub fn set_clock(&self, millis: u64) {
        self.clock_millis.set(millis);
    }

    /// Reset RNG to initial seed state (for replay).
    pub fn reset_rng(&self) {
        *self.rng.borrow_mut() = StdRng::seed_from_u64(self.seed);
    }
}

impl Environment for MockEnvironment {
    fn now_iso(&self) -> String {
        let millis = self.clock_millis.get();
        let secs = (millis / 1000) as i64;
        let nsecs = ((millis % 1000) * 1_000_000) as u32;
        let dt = DateTime::from_timestamp(secs, nsecs).expect("valid timestamp");
        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    fn now_millis(&self) -> u64 {
        self.clock_millis.get()
    }

    fn random_f64(&self) -> f64 {
        self.rng.borrow_mut().random::<f64>()
    }

    fn random_uuid(&self) -> String {
        let mut bytes = [0u8; 16];
        self.rng.borrow_mut().fill(&mut bytes);
        // Set UUID v4 variant bits
        bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1
        Uuid::from_bytes(bytes).to_string()
    }

    fn elapsed_millis(&self, since: u64) -> u64 {
        self.clock_millis.get().saturating_sub(since)
    }

    fn timestamp(&self) -> u64 {
        self.clock_millis.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_deterministic_across_instances() {
        let env1 = MockEnvironment::new(42);
        let env2 = MockEnvironment::new(42);

        assert_eq!(env1.now_millis(), env2.now_millis());
        assert_eq!(env1.now_iso(), env2.now_iso());
        assert_eq!(env1.random_f64(), env2.random_f64());
        assert_eq!(env1.random_uuid(), env2.random_uuid());
    }

    #[test]
    fn mock_rng_advances() {
        let env = MockEnvironment::new(42);
        let r1 = env.random_f64();
        let r2 = env.random_f64();
        assert_ne!(r1, r2, "RNG should advance on each call");
    }

    #[test]
    fn mock_clock_advance() {
        let env = MockEnvironment::new(42);
        let start = env.timestamp();

        assert_eq!(env.elapsed_millis(start), 0);

        env.advance_clock(5000);
        assert_eq!(env.elapsed_millis(start), 5000);

        env.advance_clock(3000);
        assert_eq!(env.elapsed_millis(start), 8000);
    }

    #[test]
    fn mock_clock_iso_format() {
        let env = MockEnvironment::new(42);
        let iso = env.now_iso();
        // Should be a valid RFC3339 timestamp
        assert!(iso.contains("T"));
        assert!(iso.ends_with("Z"));
    }

    #[test]
    fn mock_reset_rng() {
        let env = MockEnvironment::new(42);
        let r1 = env.random_f64();
        let _ = env.random_f64();
        let _ = env.random_f64();

        env.reset_rng();
        let r1_again = env.random_f64();
        assert_eq!(r1, r1_again, "RNG should replay after reset");
    }

    #[test]
    fn mock_uuid_format() {
        let env = MockEnvironment::new(42);
        let uuid = env.random_uuid();
        // Should be 36 chars: 8-4-4-4-12
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "4", "UUID version should be 4");
    }

    #[test]
    fn real_environment_basics() {
        let env = RealEnvironment::new();

        let millis = env.now_millis();
        assert!(millis > 1_700_000_000_000, "should be after 2023");

        let iso = env.now_iso();
        assert!(iso.starts_with("20"), "should start with 20xx");

        let r = env.random_f64();
        assert!((0.0..1.0).contains(&r));

        let uuid = env.random_uuid();
        assert_eq!(uuid.len(), 36);
    }

    #[test]
    fn real_elapsed() {
        let env = RealEnvironment::new();
        let start = env.timestamp();
        // Elapsed should be very small (< 100ms)
        let elapsed = env.elapsed_millis(start);
        assert!(elapsed < 100);
    }
}
