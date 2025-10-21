DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_name = 'terms'
          AND column_name = 'idf'
          AND data_type <> 'real'
    ) THEN
        ALTER TABLE terms ALTER COLUMN idf TYPE real;
    END IF;
END $$;
