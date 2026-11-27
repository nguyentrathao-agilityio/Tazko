UPDATE attachments SET mime = 'video/mp4'
WHERE kind = 'file' AND mime = 'application/octet-stream'
  AND (lower(name) LIKE '%.mp4' OR lower(name) LIKE '%.m4v' OR lower(name) LIKE '%.mov');

UPDATE attachments SET mime = 'audio/mp4'
WHERE kind = 'file' AND mime = 'application/octet-stream' AND lower(name) LIKE '%.m4a';
