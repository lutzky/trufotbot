ALTER TABLE medications ADD aliases TEXT NOT NULL DEFAULT '';
ALTER TABLE patients_medications ADD natural_input INTEGER NOT NULL DEFAULT 0;
