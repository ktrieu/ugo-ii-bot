CREATE TABLE scrum_responses (
    id INTEGER PRIMARY KEY NOT NULL,
    scrum_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    available BOOLEAN NOT NULL,
    response_time INTEGER NOT NULL,
    FOREIGN KEY (scrum_id) REFERENCES scrums(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    UNIQUE (scrum_id, user_id) ON CONFLICT ABORT
);