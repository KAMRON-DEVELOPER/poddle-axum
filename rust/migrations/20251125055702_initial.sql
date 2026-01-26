-- ==============================================
-- EXTENSIONS
-- ==============================================
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE EXTENSION IF NOT EXISTS citext;

-- ==============================================
-- HELPER FUNCTION
-- ==============================================
CREATE OR REPLACE FUNCTION trigger_set_timestamp() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = NOW();
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

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
    'running',
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

DO $$ BEGIN CREATE TYPE transaction_type AS ENUM ('free_credit', 'usage_charge', 'top_up', 'refund');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN CREATE TYPE provider AS ENUM ('google', 'github', 'email');
EXCEPTION
WHEN duplicate_object THEN NULL;
END $$;

-- ==============================================
-- SYSTEM CONFIG (controls free credit dynamically)
-- ==============================================
CREATE TABLE IF NOT EXISTS system_config (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE,
    free_credit_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    free_credit_amount NUMERIC(18, 2) NOT NULL DEFAULT 50000.00,
    free_credit_detail TEXT DEFAULT 'Free credit',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_system_config_timestamp BEFORE UPDATE ON system_config FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

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
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_oauth_email UNIQUE (email)
);

CREATE TRIGGER set_oauth_users_timestamp BEFORE UPDATE ON oauth_users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- ==============================================
-- USERS
-- ==============================================
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    username VARCHAR(64) NOT NULL,
    email VARCHAR(255) NOT NULL,
    password TEXT,
    picture TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    role user_role NOT NULL DEFAULT 'regular',
    status user_status NOT NULL DEFAULT 'pending_verification',
    oauth_user_id VARCHAR(255) REFERENCES oauth_users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (username)
);

CREATE TRIGGER set_users_timestamp BEFORE UPDATE ON users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email ON users (lower(email));

-- ==============================================
-- SESSIONS
-- ==============================================
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_agent TEXT,
    ip_address VARCHAR(45),
    device_name VARCHAR(100),
    refresh_token TEXT UNIQUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_sessions_timestamp BEFORE UPDATE ON sessions FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);

CREATE INDEX IF NOT EXISTS idx_sessions_is_active ON sessions (is_active);

-- ==============================================
-- PROJECTS
-- ==============================================
CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    owner_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name VARCHAR(150) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (owner_id, name)
);

CREATE TRIGGER set_projects_timestamp BEFORE UPDATE ON projects FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- ==============================================
-- PRESETS
-- ==============================================
CREATE TABLE IF NOT EXISTS presets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    name VARCHAR(50) NOT NULL UNIQUE,
    description TEXT,
    -- Resources (What is included in the plan)
    cpu_millicores INTEGER NOT NULL CHECK (cpu_millicores > 0),
    memory_mb INTEGER NOT NULL CHECK (memory_mb > 0),
    -- Pricing
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    monthly_price NUMERIC(18, 2) NOT NULL CHECK (monthly_price >= 0),
    hourly_price NUMERIC(18, 6) NOT NULL GENERATED ALWAYS AS (monthly_price / 720.0) STORED,
    -- Guardrails (Thresholds for Add-ons)
    max_addon_cpu_millicores INTEGER NOT NULL DEFAULT 0,
    max_addon_memory_mb INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_presets_timestamp BEFORE UPDATE ON presets FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- ==============================================
-- ADDON PRICES
-- ==============================================
CREATE TABLE addon_prices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    cpu_monthly_unit_price NUMERIC(18, 6) NOT NULL,
    cpu_hourly_unit_price NUMERIC(18, 6) NOT NULL GENERATED ALWAYS AS (
        cpu_monthly_unit_price / 720.0
    ) STORED,
    memory_monthly_unit_price NUMERIC(18, 6) NOT NULL,
    memory_hourly_unit_price NUMERIC(18, 6) NOT NULL GENERATED ALWAYS AS (
        memory_monthly_unit_price / 720.0
    ) STORED,
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ==============================================
-- DEPLOYMENTS
-- ==============================================

CREATE TABLE IF NOT EXISTS deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    image VARCHAR(500) NOT NULL,
    port INT NOT NULL,
    desired_replicas INTEGER NOT NULL DEFAULT 1 CHECK (desired_replicas >= 1),
    ready_replicas INTEGER NOT NULL,
    available_replicas INTEGER NOT NULL,
    preset_id UUID NOT NULL REFERENCES presets (id) ON DELETE CASCADE,
    addon_cpu_millicores INTEGER CHECK (addon_cpu_millicores >= 0),
    addon_memory_mb INTEGER CHECK (addon_memory_mb >= 0),
    vault_secret_path VARCHAR(250),
    secret_keys VARCHAR(64) [],
    environment_variables JSONB,
    labels JSONB,
    status deployment_status NOT NULL DEFAULT 'queued',
    domain VARCHAR(253),
    subdomain VARCHAR(63),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_deployments_timestamp BEFORE UPDATE ON deployments FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE INDEX IF NOT EXISTS idx_deployments_user_id ON deployments (user_id);

CREATE INDEX IF NOT EXISTS idx_deployments_project_id ON deployments (project_id);

CREATE INDEX IF NOT EXISTS idx_deployments_status ON deployments (status);

-- ==============================================
-- DEPLOYMENT EVENTS
-- ==============================================
CREATE TABLE IF NOT EXISTS deployment_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    deployment_id UUID NOT NULL REFERENCES deployments (id) ON DELETE CASCADE,
    type VARCHAR(128) NOT NULL,
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_deployment_events_timestamp BEFORE UPDATE ON deployment_events FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE INDEX IF NOT EXISTS idx_deployment_events_deployment_id ON deployment_events (deployment_id);

CREATE INDEX IF NOT EXISTS idx_deployment_events_created ON deployment_events (created_at DESC);

-- ==============================================
-- BILLINGS
-- ==============================================
CREATE TABLE IF NOT EXISTS billings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE SET NULL,
    deployment_id UUID REFERENCES deployments (id) ON DELETE SET NULL,
    -- Scaling Factor
    desired_replicas INTEGER NOT NULL DEFAULT 1,
    -- PRESET SNAPSHOT
    preset_cpu_millicores INTEGER NOT NULL,
    preset_memory_mb INTEGER NOT NULL,
    preset_hourly_price NUMERIC(18, 6) NOT NULL,
    -- ADDON SNAPSHOT
    addon_cpu_millicores INTEGER NOT NULL DEFAULT 0,
    addon_memory_mb INTEGER NOT NULL DEFAULT 0,
    addon_cpu_millicores_hourly_price NUMERIC(18, 6) NOT NULL,
    addon_memory_mb_hourly_price NUMERIC(18, 6) NOT NULL,
    -- TOTAL USAGE  
    cpu_millicores_used INTEGER GENERATED ALWAYS AS (
        (
            preset_cpu_millicores + addon_cpu_millicores
        ) * desired_replicas
    ) STORED,
    memory_mb_used INTEGER GENERATED ALWAYS AS (
        (
            preset_memory_mb + addon_memory_mb
        ) * desired_replicas
    ) STORED,
    -- TIME SLICE
    hours_used NUMERIC(18, 6) NOT NULL,
    -- TOTAL
    total_cost NUMERIC(18, 6) GENERATED ALWAYS AS (
        (
            preset_hourly_price + addon_cpu_millicores * addon_cpu_millicores_hourly_price + addon_memory_mb * addon_memory_mb_hourly_price
        ) * desired_replicas * hours_used
    ) STORED,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_billing_timestamp BEFORE UPDATE ON billings FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE INDEX IF NOT EXISTS idx_billings_user_id ON billings (user_id);

CREATE INDEX IF NOT EXISTS idx_billings_deployment_id ON billings (deployment_id);

-- ==============================================
-- BALANCES
-- ==============================================
CREATE TABLE IF NOT EXISTS balances (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    user_id UUID NOT NULL UNIQUE REFERENCES users (id) ON DELETE SET NULL,
    amount NUMERIC(18, 6) NOT NULL DEFAULT 0.000000,
    currency CHAR(3) NOT NULL DEFAULT 'UZS',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_balances_timestamp BEFORE UPDATE ON balances FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- ==============================================
-- TRANSACTIONS (The Ledger)
-- ==============================================
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    balance_id UUID NOT NULL REFERENCES balances (id) ON DELETE CASCADE,
    billing_id UUID REFERENCES billings (id) ON DELETE SET NULL,
    amount NUMERIC(18, 6) NOT NULL, -- Negative for charges, Positive for deposits
    detail TEXT,
    type transaction_type NOT NULL,
    external_transaction_id VARCHAR(255), -- ID from Payme / Click / Uzum
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER set_transactions_timestamp BEFORE UPDATE ON transactions FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE INDEX IF NOT EXISTS idx_transactions_balance_id ON transactions (balance_id);

-- Prevent duplicate payments
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_external_id ON transactions (external_transaction_id)
WHERE
    external_transaction_id IS NOT NULL;

-- ==============================================
-- AUTO-BALANCE CREATION + OPTIONAL FREE CREDIT
-- ==============================================
CREATE OR REPLACE FUNCTION on_user_insert_create_balance() RETURNS TRIGGER AS $$
DECLARE
    cfg RECORD;
    balance_id UUID;
BEGIN INSERT INTO balances (user_id, amount) VALUES (NEW.id, 0.00) RETURNING id INTO balance_id;
SELECT * INTO cfg FROM system_config LIMIT 1;
IF cfg.free_credit_enabled AND cfg.free_credit_amount > 0 THEN
    INSERT INTO transactions (balance_id, amount, type, detail)
    VALUES (balance_id, cfg.free_credit_amount, 'free_credit', cfg.free_credit_detail);
END IF;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER after_user_inserted AFTER INSERT ON users FOR EACH ROW EXECUTE PROCEDURE on_user_insert_create_balance();

-- ==============================================
-- APPLY TRANSACTION TO BALANCE
-- ==============================================
CREATE OR REPLACE FUNCTION on_transaction_insert_deduct_balance() RETURNS TRIGGER AS $$
DECLARE 
    current_balance NUMERIC(18, 6);
    new_balance NUMERIC(18, 6);
BEGIN
    SELECT amount INTO current_balance FROM balances WHERE id = NEW.balance_id FOR UPDATE;
    
    new_balance := (current_balance + NEW.amount);
    
    -- Optional: Allow negative balance for 'usage_charge' but not for 'top_up'
    -- This prevents race conditions where a user spends money they don't have
    IF new_balance < 0 AND NEW.type != 'usage_charge' THEN 
        RAISE EXCEPTION 'Insufficient funds. Transaction aborted.';
    END IF;

    UPDATE balances SET amount = new_balance, updated_at = NOW() WHERE id = NEW.balance_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER after_transaction_insert AFTER INSERT ON transactions FOR EACH ROW EXECUTE PROCEDURE on_transaction_insert_deduct_balance();

-- ==============================================
-- SEED DEPLOYMENT PRESETS DATA
-- ==============================================
INSERT INTO
    presets (
        name,
        cpu_millicores,
        memory_mb,
        description,
        currency,
        monthly_price
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

-- ==============================================
-- SEED RESOURCE RATES DATA
-- ==============================================
INSERT INTO
    addon_prices (
        cpu_monthly_unit_price,
        memory_monthly_unit_price
    )
VALUES (20000, 15000);