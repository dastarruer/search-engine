DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'pages' AND column_name = 'html'
    ) THEN
        ALTER TABLE pages ALTER COLUMN html DROP NOT NULL;
    END IF;
END $$;
