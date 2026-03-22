-- ── Vendor role setup ───────────────────────────────────────────
-- Run this against the shared auth_db database.
-- This migration is owned by the vendor-api service but modifies
-- shared tables — both services read from the same schema.

-- Add role to users (default reader, can be promoted to vendor/admin)
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'reader'
    CHECK (role IN ('reader', 'vendor', 'admin'));

-- Link bookstores to their owner vendor account
ALTER TABLE bookstores
    ADD COLUMN IF NOT EXISTS owner_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Index for fast vendor order lookups
CREATE INDEX IF NOT EXISTS idx_bookstores_owner
    ON bookstores(owner_id)
    WHERE owner_id IS NOT NULL;

-- Index for role-based auth checks
CREATE INDEX IF NOT EXISTS idx_users_role
    ON users(role)
    WHERE role != 'reader';