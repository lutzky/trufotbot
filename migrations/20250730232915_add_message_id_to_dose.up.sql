-- Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
--
-- SPDX-License-Identifier: GPL-3.0-only

-- Although 'patients' has a telegram_group_id, this might have changed since
-- the dose was given.
ALTER TABLE doses ADD telegram_group_id INTEGER;
ALTER TABLE doses ADD telegram_message_id INTEGER;
