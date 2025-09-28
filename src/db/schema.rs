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
        sleep_interval -> Integer,
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

diesel::allow_tables_to_appear_in_same_query!(dyndns, history,);
