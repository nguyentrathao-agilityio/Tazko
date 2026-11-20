CREATE INDEX events_card ON events (card_id, id) WHERE card_id IS NOT NULL;
