
CREATE TABLE pairs (
  id              INTEGER  GENERATED ALWAYS AS IDENTITY,
  base         VARCHAR(10) NOT NULL,
  quote        VARCHAR(10) NOT NULL,
  name         VARCHAR(20) NOT NULL,
  enabled         BOOLEAN  NOT NULL DEFAULT TRUE,
  PRIMARY KEY(id)
);


CREATE UNIQUE INDEX pairs_name_idx ON pairs (name);

CREATE TABLE ticks (
  timestamp  TIMESTAMP NOT NULL DEFAULT NOW(),
  pair_id INTEGER NOT NULL,
  open DECIMAL(16, 8) NOT NULL,
  close DECIMAL(16, 8) NOT NULL,
  low DECIMAL(16, 8) NOT NULL,
  high DECIMAL(16, 8) NOT NULL,
  volume DECIMAL(16, 2) NOT NULL,
  orders SERIAL NOT NULL,
  PRIMARY KEY (timestamp, pair_id)

);

ALTER TABLE ticks ADD CONSTRAINT fk_ticks_pairs
    FOREIGN KEY (pair_id)
      REFERENCES pairs(id)
      on delete cascade
      on update cascade ;

CREATE INDEX ticks_pair_id_idx ON public.ticks (pair_id,volume);
