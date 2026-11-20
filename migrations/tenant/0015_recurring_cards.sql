ALTER TABLE cards ADD COLUMN recurrence TEXT CHECK (recurrence IN ('daily', 'weekdays', 'weekly', 'biweekly', 'monthly'));
