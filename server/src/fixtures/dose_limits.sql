INSERT INTO patients (name, telegram_group_id) VALUES
    ('Alice', -123),
    ('Bob', -123),
    ('Carol', -123);

INSERT INTO medications (name, description, dose_limits) VALUES
    ('Paracetamol', 'Pain reliever and fever reducer', '4:2, 24:8');

-- Note: Current time is 2023-04-05T07:07:08

INSERT INTO doses (patient_id, medication_id, quantity, taken_at) VALUES
    -- Alice
    (1, 1, 2.0, "2023-04-05T04:00:00Z"),
    -- Bob
    (2, 1, 2.0, "2023-04-01T00:00:00Z"),
    -- Carol
    (3, 1, 2.0, "2023-04-01T00:00:00Z"),
    (3, 1, 2.0, "2023-04-05T00:00:00Z"),
    (3, 1, 2.0, "2023-04-05T01:00:00Z"),
    (3, 1, 2.0, "2023-04-05T02:00:00Z"),
    (3, 1, 1.0, "2023-04-05T03:00:00Z");
