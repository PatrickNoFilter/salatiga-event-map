CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE event_status AS ENUM ('pending', 'approved', 'rejected', 'cancelled');

CREATE TABLE events (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  title        TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  category     TEXT NOT NULL,
  starts_at    TIMESTAMPTZ NOT NULL,
  ends_at      TIMESTAMPTZ,
  venue_name   TEXT NOT NULL,
  address      TEXT NOT NULL DEFAULT '',
  lat          DOUBLE PRECISION NOT NULL,
  lng          DOUBLE PRECISION NOT NULL,
  price_text   TEXT NOT NULL DEFAULT 'Free',
  is_free      BOOLEAN NOT NULL DEFAULT TRUE,
  organizer    TEXT NOT NULL DEFAULT '',
  contact      TEXT NOT NULL DEFAULT '',
  ticket_url   TEXT,
  image_url    TEXT,
  tags         TEXT[] NOT NULL DEFAULT '{}',
  status       event_status NOT NULL DEFAULT 'pending',
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  geom         GEOGRAPHY(Point, 4326)
               GENERATED ALWAYS AS (ST_SetSRID(ST_MakePoint(lng, lat), 4326)::geography) STORED
);

CREATE INDEX events_geom_idx   ON events USING GIST (geom);
CREATE INDEX events_starts_idx ON events (starts_at);
CREATE INDEX events_status_idx ON events (status);
