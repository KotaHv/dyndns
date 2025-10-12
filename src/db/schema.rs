// @generated automatically by Diesel CLI.

diesel::table! {
    dyndns (id) {
        id -> Integer,
        server -> Text,
        username -> Text,
        password -> Text,
        hostname -> Text,
        ip -> Integer,
        interface -> Text,
        sleep_interval -> BigInt,
    }
}

diesel::table! {
    history (id) {
        id -> Integer,
        old_ip -> Nullable<Text>,
        new_ip -> Text,
        version -> Integer,
        updated -> Timestamp,
    }
}

diesel::table! {
    refresh_tokens (selector) {
        selector -> Text,
        verifier_hash -> Text,
        expires_at -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::table! {
    auth_secrets (id) {
        id -> Integer,
        secret -> Text,
        created_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(dyndns, history, refresh_tokens, auth_secrets,);
