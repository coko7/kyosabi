use std::sync::Arc;

use axum::body::Body;
use governor::middleware::NoOpMiddleware;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::PeerIpKeyExtractor;

/// watari.md §12 names which routes need rate limiting (`/auth/callback`,
/// `POST /api/upload`, `/api/paste`, `/api/shorten`) but not exact quotas —
/// these are reasonable defaults, keyed on peer IP (not spoofable headers).
type Governor = GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware, Body>;

/// For `/auth/callback`: low burst, since a legit user only completes login once.
pub fn auth_governor_layer() -> Governor {
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(4)
            .burst_size(2)
            .finish()
            .expect("static governor config is always valid"),
    );
    GovernorLayer::new(config)
}

/// For the mutating upload/paste/shorten API routes: looser, since a user
/// legitimately submits several of these per session.
pub fn api_governor_layer() -> Governor {
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(10)
            .finish()
            .expect("static governor config is always valid"),
    );
    GovernorLayer::new(config)
}
