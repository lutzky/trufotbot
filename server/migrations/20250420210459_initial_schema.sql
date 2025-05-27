CREATE TABLE patients (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_group_id INTEGER,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE medications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    dose_limits TEXT
);

CREATE TABLE reminders (
    patient_id INTEGER NOT NULL,
    medication_id INTEGER NOT NULL,
    cron_schedule TEXT NOT NULL, -- Newline-delimited cron schedules
    FOREIGN KEY (patient_id) REFERENCES patients(id),
    FOREIGN KEY (medication_id) REFERENCES medications(id),
    PRIMARY KEY (patient_id, medication_id)
);

CREATE TABLE doses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    medication_id INTEGER NOT NULL,
    quantity REAL NOT NULL,
    taken_at DATETIME NOT NULL,
    noted_by_user TEXT, -- Optional: Who recorded this dose
    FOREIGN KEY (patient_id) REFERENCES patients(id),
    FOREIGN KEY (medication_id) REFERENCES medications(id)
);
