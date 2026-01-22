-- ==============================================
-- EXTENSIONS
-- ==============================================
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS citext;
--
--
-- ==============================================
-- HELPER FUNCTION
-- ==============================================
CREATE OR REPLACE FUNCTION trigger_set_timestamp() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = NOW();
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
--
--
-- ==============================================
-- ENUM TYPES
-- ==============================================
DO $$ BEGIN CREATE TYPE deployment_status AS ENUM (
    -- 1. Saved in DB, waiting for Worker
    'queued',
    -- 2. Worker is creating K8s resources
    'provisioning',
    -- 3. K8s accepted it, pulling images (ContainerCreating)
    'starting',
    -- 4. All pods are Running and Ready
    'healthy',
    -- 5. Pods are crashing or failing health checks
    'unhealthy',
    -- 6. Expected 3 replicas, but only 2 are ready
    'degraded',
    -- 7. Updating some paramert of the deployment
    'updating',
    -- 8. Stopped by user or billing
    'suspended',
    -- 9. Configuration error (Image pull backoff, etc.)
    'failed'
);
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;
DO $$ BEGIN CREATE TYPE user_role AS ENUM ('admin', 'regular');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;
DO $$ BEGIN CREATE TYPE user_status AS ENUM ('active', 'suspended', 'pending_verification');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;
DO $$ BEGIN CREATE TYPE transaction_type AS ENUM ('free_credit', 'usage_charge', 'fund');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;
DO $$ BEGIN CREATE TYPE provider AS ENUM ('google', 'github', 'email');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;
--
--
-- =====================
-- OAUTH USERS
-- =====================
CREATE TABLE IF NOT EXISTS oauth_users (
    id VARCHAR(255) PRIMARY KEY,
    provider provider NOT NULL,
    username VARCHAR(50),
    email VARCHAR(100),
    password TEXT,
    picture TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_oauth_email UNIQUE(email)
);
CREATE TRIGGER set_oauth_users_timestamp BEFORE
UPDATE ON oauth_users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
--
--
-- ==============================================
-- USERS
-- ==============================================
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(64) NOT NULL,
    email VARCHAR(255) NOT NULL,
    password TEXT NOT NULL,
    picture TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    role user_role NOT NULL DEFAULT 'regular',
    status user_status NOT NULL DEFAULT 'pending_verification',
    oauth_user_id VARCHAR(255) REFERENCES oauth_users(id) ON DELETE
    SET NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        UNIQUE (username)
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_lower_email ON users (lower(email));
CREATE TRIGGER set_users_timestamp BEFORE
UPDATE ON users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
--
--
-- ==============================================
-- SESSIONS
-- ==============================================
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent TEXT,
    ip_address VARCHAR(45),
    device_name VARCHAR(100),
    refresh_token TEXT UNIQUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_user_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_sessions_active ON sessions(is_active);
--
--
-- ==============================================
-- BALANCES
-- ==============================================
CREATE TABLE IF NOT EXISTS balances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    amount NUMERIC(18, 6) NOT NULL DEFAULT 0.000000,
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TRIGGER set_balances_timestamp BEFORE
UPDATE ON balances FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
--
--
-- ==============================================
-- PROJECTS
-- ==============================================
CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(150) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, name)
);
CREATE TRIGGER set_projects_timestamp BEFORE
UPDATE ON projects FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
--
--
-- ==============================================
-- DEPLOYMENTS
-- ==============================================
CREATE TABLE IF NOT EXISTS deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    image VARCHAR(500) NOT NULL,
    replicas INTEGER NOT NULL DEFAULT 1 CHECK (replicas >= 1),
    port INT NOT NULL,
    vault_secret_path VARCHAR(250),
    secret_keys VARCHAR(64) [],
    -- environment_variables JSONB DEFAULT '{}'::jsonb
    environment_variables JSONB,
    -- resources JSONB NOT NULL DEFAULT '{"cpuRequestMillicores":250,"cpuLimitMillicores":500,"memoryRequestMb":256,"memoryLimitMb":512}'::jsonb
    resources JSONB NOT NULL,
    labels JSONB,
    status deployment_status NOT NULL DEFAULT 'queued',
    subdomain VARCHAR(63),
    custom_domain VARCHAR(253),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_deployments_user_id ON deployments(user_id);
CREATE INDEX IF NOT EXISTS idx_deployments_project_id ON deployments(project_id);
CREATE INDEX IF NOT EXISTS idx_deployments_status ON deployments(status);
CREATE TRIGGER set_deployments_timestamp BEFORE
UPDATE ON deployments FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
--
--
-- ==============================================
-- DEPLOYMENT EVENTS
-- ==============================================
CREATE TABLE IF NOT EXISTS deployment_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    deployment_id UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    event_type VARCHAR(128) NOT NULL,
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_deployment_events_deployment_id ON deployment_events(deployment_id);
CREATE INDEX IF NOT EXISTS idx_deployment_events_created ON deployment_events(created_at DESC);
--
--
-- ==============================================
-- BILLINGS
-- ==============================================
CREATE TABLE IF NOT EXISTS billings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    deployment_id UUID REFERENCES deployments(id) ON DELETE
    SET NULL,
        resources_snapshot JSONB NOT NULL,
        cpu_millicores INTEGER NOT NULL CHECK (cpu_millicores >= 0),
        memory_mb INTEGER NOT NULL CHECK (memory_mb >= 0),
        cost_per_hour NUMERIC(18, 8) NOT NULL,
        hours_used NUMERIC(12, 6) NOT NULL DEFAULT 1.0,
        total_cost NUMERIC(20, 8) GENERATED ALWAYS AS (cost_per_hour * hours_used) STORED,
        created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_billings_user_id ON billings(user_id);
CREATE INDEX IF NOT EXISTS idx_billings_deployment_id ON billings(deployment_id);
--
--
-- ==============================================
-- TRANSACTIONS
-- ==============================================
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    balance_id UUID NOT NULL REFERENCES balances(id) ON DELETE CASCADE,
    amount NUMERIC(18, 6) NOT NULL,
    type transaction_type NOT NULL,
    -- ID from Payme / Click / Uzum
    external_transaction_id VARCHAR(255),
    detail TEXT,
    billing_id UUID REFERENCES billings(id) ON DELETE
    SET NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_transactions_balance_id ON transactions(balance_id);
-- Create a unique index so the same payment ID from Payme/Click/uzum can never be inserted twice
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_external_transaction_id ON transactions(external_transaction_id)
WHERE external_transaction_id IS NOT NULL;
--
--
-- ==============================================
-- APPLY TRANSACTION TO BALANCE
-- ==============================================
CREATE OR REPLACE FUNCTION apply_transaction() RETURNS TRIGGER AS $$
DECLARE current_balance NUMERIC(18, 6);
new_balance NUMERIC(18, 6);
BEGIN
SELECT amount INTO current_balance
FROM balances
WHERE id = NEW.balance_id FOR
UPDATE;
IF NOT FOUND THEN RAISE EXCEPTION 'Balance % not found',
NEW.balance_id;
END IF;
new_balance := (current_balance + NEW.amount);
IF new_balance < 0 THEN RAISE EXCEPTION 'Insufficient funds: balance % would go negative (current=%). Transaction aborted.',
NEW.balance_id,
current_balance;
END IF;
UPDATE balances
SET amount = new_balance,
    updated_at = NOW()
WHERE id = NEW.balance_id;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER after_transaction_insert
AFTER
INSERT ON transactions FOR EACH ROW EXECUTE PROCEDURE apply_transaction();
--
--
-- ==============================================
-- SYSTEM CONFIG (controls free credit dynamically)
-- ==============================================
CREATE TABLE IF NOT EXISTS system_config (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE,
    free_credit_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    free_credit_amount NUMERIC(18, 6) NOT NULL DEFAULT 0.00,
    free_credit_detail TEXT DEFAULT 'Free credit',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
--
--
-- ==============================================
-- AUTO-BALANCE CREATION + OPTIONAL FREE CREDIT
-- ==============================================
CREATE OR REPLACE FUNCTION on_user_insert_create_balance() RETURNS TRIGGER AS $$
DECLARE cfg RECORD;
balance_id UUID;
BEGIN -- Create empty balance for user
INSERT INTO balances (user_id, amount)
VALUES (NEW.id, 0.00)
RETURNING id INTO balance_id;
-- Load current system config
SELECT * INTO cfg
FROM system_config
LIMIT 1;
-- If free credit is enabled, apply it
IF cfg.free_credit_enabled
AND cfg.free_credit_amount > 0 THEN
INSERT INTO transactions (balance_id, amount, type, detail)
VALUES (
        balance_id,
        cfg.free_credit_amount,
        'free_credit',
        cfg.free_credit_detail
    );
END IF;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER after_user_inserted
AFTER
INSERT ON users FOR EACH ROW EXECUTE PROCEDURE on_user_insert_create_balance();
--
--
-- ==============================================
-- DEPLOYMENT PRESETS (Plans)
-- ==============================================
CREATE TABLE deployment_presets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(50) NOT NULL,
    cpu_millicores INTEGER NOT NULL CHECK (cpu_millicores > 0),
    memory_mb INTEGER NOT NULL CHECK (memory_mb > 0),
    description TEXT,
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    price_per_month NUMERIC(18, 2) NOT NULL CHECK (price_per_month >= 0),
    price_per_hour NUMERIC(10, 4) NOT NULL CHECK (price_per_hour >= 0) GENERATED ALWAYS AS (price_per_month / 720) STORED,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- ensure only one default preset
CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_presets_one_default ON deployment_presets(is_default)
WHERE is_default = TRUE;
--
--
-- ==============================================
-- RESOURCE RATE
-- ==============================================
CREATE TABLE resource_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    resource_type VARCHAR(32) NOT NULL,
    price_per_unit_hour NUMERIC(18, 8) NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    effective_from TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    effective_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
--
--
-- ==============================================
-- SEED DATA (Plans)
-- ==============================================
INSERT INTO deployment_presets (
        name,
        cpu_millicores,
        memory_mb,
        description,
        currency,
        price_per_month,
    )
VALUES (
        'Starter',
        100,
        128,
        'Perfect for testing and small projects',
        'UZS',
        12000
    ),
    (
        'Sandbox',
        200,
        256,
        'Development
and staging environments',
        'UZS',
        20000
    ),
    (
        'Standard',
        500,
        512,
        'Small production workloads',
        'UZS',
        35000
    ),
    (
        'Growth',
        1000,
        1024,
        'Growing applications',
        'UZS',
        50000
    ),
    (
        'Business',
        2000,
        2048,
        'Business applications',
        'UZS',
        85000
    ),
    (
        'Pro',
        4000,
        4096,
        'High - performance applications',
        'UZS',
        180000
    ),
    (
        'Enterprise',
        4000,
        8192,
        'High - Enterprise workloads',
        'UZS',
        210000
    );