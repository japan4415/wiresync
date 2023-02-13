use sqlx::postgres::{PgPool, PgPoolOptions};
use std::error::Error;
use tracing::{debug, error, info};

pub async fn make_pool(
    endpoint: &String,
    port: i32,
    user: &String,
    password: &String,
) -> Result<PgPool, Box<dyn Error>> {
    let postgres_url = format!(
        "postgres://{}:{}/wiresync?user={}&password={}",
        endpoint, port, user, password
    );
    info!("{}", postgres_url);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&postgres_url)
        .await?;
    Ok(pool)
}

pub async fn init_db(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS peerdata (
            endpoint varchar(100) NOT NULL,
            port int NOT NULL,
            privateKey varchar(100) NOT NULL,
            nicName varchar(10) NOT NULL,
            inWgIp varchar(15) NOT NULL
        );
    ",
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug)]
pub struct ServerData {
    pub endpoint: String,
    pub port: i32,
    pub privatekey: String,
    pub nicname: String,
    pub inwgip: String,
}
pub async fn get_interface_data(
    endpoint: &String,
    port: &i32,
    pool: &PgPool,
) -> Result<ServerData, Box<dyn Error>> {
    let data = sqlx::query_as!(
        ServerData,
        r#"SELECT * FROM peerdata WHERE endpoint = $1 AND port = $2"#,
        endpoint,
        port,
    )
    .fetch_one(pool)
    .await?;
    Ok(data)
}
pub async fn get_peer_datas(
    endpoint: &String,
    port: &i32,
    pool: &PgPool,
) -> Result<Vec<ServerData>, Box<dyn Error>> {
    let datas = sqlx::query_as!(
        ServerData,
        r#"SELECT * FROM peerdata WHERE endpoint != $1 AND port != $2"#,
        endpoint,
        port
    )
    .fetch_all(pool)
    .await?;
    Ok(datas)
}

#[derive(Debug)]
struct IpResult {
    inwgip: String,
}
pub async fn get_all_ip(pool: &PgPool) -> Result<Vec<(i32, i32, i32, i32)>, Box<dyn Error>> {
    let ip_result = sqlx::query_as!(IpResult, r#"SELECT inWgIp FROM peerdata"#,)
        .fetch_all(pool)
        .await?;
    let mut new_ips: Vec<(i32, i32, i32, i32)> = vec![];
    for ip in &ip_result {
        let splited_ip: Vec<&str> = ip.inwgip.split(".").collect();
        let ip_tuple: (i32, i32, i32, i32) = (
            splited_ip[0].parse().unwrap(),
            splited_ip[1].parse().unwrap(),
            splited_ip[2].parse().unwrap(),
            splited_ip[3].parse().unwrap(),
        );
        new_ips.push(ip_tuple);
    }
    Ok(new_ips)
}

#[derive(Debug)]
struct CountResult {
    count: Option<i64>,
}
pub async fn check_duplication(
    pool: &PgPool,
    endpoint: &String,
    port: &i32,
) -> Result<bool, Box<dyn Error>> {
    let count_result = sqlx::query_as!(
        CountResult,
        r#"SELECT count(endpoint) FROM peerdata WHERE endpoint = $1 AND port = $2"#,
        endpoint,
        port
    )
    .fetch_one(pool)
    .await?;
    if count_result.count.unwrap() == 0 {
        return Ok(true);
    } else {
        return Ok(false);
    }
}

pub async fn check_change(
    pool: &PgPool,
    endpoint: &String,
    port: &i32,
    private_key: &String,
    nic_name: &String,
    in_wg_ip: &String,
) -> Result<bool, Box<dyn Error>> {
    let count_result = sqlx::query_as!(
        CountResult,
        r#"
        SELECT count(endpoint) 
        FROM peerdata 
        WHERE endpoint = $1 
        AND port = $2 
        AND privateKey = $3
        AND nicName = $4
        AND inWgIp = $5
        "#,
        endpoint,
        port,
        private_key,
        nic_name,
        in_wg_ip,
    )
    .fetch_one(pool)
    .await?;
    if count_result.count.unwrap() == 0 {
        return Ok(true);
    } else {
        return Ok(false);
    }
}

pub async fn submit_new_server(
    pool: &PgPool,
    endpoint: &String,
    port: &i32,
    private_key: &String,
    nic_name: &String,
    in_wg_ip: &String,
) -> Result<(), Box<dyn Error>> {
    let query = format!(
        "
        INSERT INTO peerdata
        VALUES('{}', {}, '{}', '{}', '{}')
    ",
        endpoint, port, private_key, nic_name, in_wg_ip
    );
    sqlx::query(&query).execute(pool).await?;
    Ok(())
}

pub async fn delete_server(
    pool: &PgPool,
    endpoint: &String,
    port: &i32,
) -> Result<(), Box<dyn Error>> {
    let query = format!(
        "
        DROP FROM peerdata 
        WHERE endpoint = {} 
        AND port = {}
    ",
        endpoint, port,
    );
    sqlx::query(&query).execute(pool).await?;
    Ok(())
}

pub async fn update_server(
    pool: &PgPool,
    endpoint: &String,
    port: &i32,
    private_key: &String,
    nic_name: &String,
    in_wg_ip: &String,
) -> Result<(), Box<dyn Error>> {
    let query = format!(
        "
        UPDATE peerdata
        SET (endpoint, port, privateKey, nicName, inWgIp) = ('{}', {}, '{}', '{}', '{}')
    ",
        endpoint, port, private_key, nic_name, in_wg_ip
    );
    sqlx::query(&query).execute(pool).await?;
    Ok(())
}
