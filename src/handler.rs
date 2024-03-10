use std::error::Error;

use ntex::web::{self, DefaultError};
use r2d2::Pool;
use r2d2_postgres::{postgres::NoTls, PostgresConnectionManager};
use serde::{Deserialize, Serialize};

type DbConnection = PostgresConnectionManager<NoTls>;

type DbPool = Pool<DbConnection>;

#[derive(Deserialize)]
struct Payload {
    valor: i64,
    tipo: String,
    descricao: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Conta {
    saldo: i64,
    limite: i64,
}

#[derive(Serialize)]
struct Extrato {
    saldo: Saldo,
    ultimas_transacoes: Vec<Transacao>,
}

#[derive(Serialize)]
struct Saldo {
    total: i64,
    data_extrato: String,
    limite: i64,
}

#[derive(Serialize)]
struct Transacao {
    valor: i64,
    tipo: String,
    descricao: String,
    realizada_em: String,
}

static ACCOUNT_SQL: &str = "
SELECT
balance,
credit,
TO_CHAR(NOW(), 'YYYY-MM-DD\"T\"HH24:MI:SS.MSZ')
FROM accounts
WHERE id=$1
";

static ACCOUNT_SQL_FOR_UPDATE: &str = "
SELECT
balance,
credit
FROM accounts
WHERE id=$1
FOR UPDATE
";

static LAST_TRANSACTION_SQL: &str = "
SELECT
tx.amount,
tx.transaction_type,
tx.details,
TO_CHAR(tx.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.MSZ')
FROM transactions tx
WHERE tx.account_id = $1 ORDER BY created_at DESC LIMIT 10
";

#[web::get("/{id}/extrato")]
async fn get_balance(
    path: web::types::Path<i32>,
    db: web::types::State<DbPool>,
) -> Result<impl web::Responder, web::Error> {
    let id = path.into_inner();
    let db = db.get_ref().clone();
    let res = web::block(move || -> Result<Extrato, i32> {
        let mut conn = db.get().unwrap();
        let rows: Vec<(i64,i64,String)> = conn
            .query(ACCOUNT_SQL, &[&id])
            .unwrap()
            .iter()
            .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect();
        if rows.is_empty() {
            return Err(404);
        }
        let conta = rows.get(0).unwrap();
        let ultimas_transacoes = conn
            .query(LAST_TRANSACTION_SQL, &[&id])
            .unwrap()
            .iter()
            .map(|row| Transacao {
                valor: row.get(0),
                tipo: row.get(1),
                descricao: row.get(2),
                realizada_em: row.get(3),
            })
            .collect();
        Ok(Extrato {
            saldo: Saldo {
                total: conta.0,
                limite: conta.1,
                data_extrato: conta.2.clone(),
            },
            ultimas_transacoes,
        })
    })
    .await
    .map(|extrato| ntex::web::HttpResponse::Ok().json(&extrato))
    .map_err(|err| {
        let err_str = format!("{}", err);
        match err_str.as_str() {
            "404" => web::error::ErrorNotFound(err),
            _ => web::error::ErrorUnprocessableEntity(err),
        }
    });
    Ok(res)
}

#[web::post("/{id}/transacoes")]
async fn post_transaction(
    path: web::types::Path<i32>,
    payload: web::types::Json<Payload>,
    db: web::types::State<DbPool>,
) -> Result<impl web::Responder, web::Error> {
    let id = path.into_inner();
    let payload = payload.into_inner();
    let descricao = match payload.descricao {
        Some(val) => val,
        None => {
            return Ok(web::HttpResponse::UnprocessableEntity().body("descricao nulo"));
        }
    };
    if descricao.is_empty() || descricao.len() > 10 {
        return Ok(web::HttpResponse::UnprocessableEntity().body("descricao vazio ou maior que 10"));
    }
    let valor = match payload.tipo.as_str() {
        "c" => payload.valor,
        "d" => payload.valor * -1,
        _ => {
            return Ok(web::HttpResponse::UnprocessableEntity().body("Error"));
        }
    };
    let db = db.get_ref().clone();
    let res = web::block(move || {
        let mut conn = db.get().unwrap();
        let mut tx = conn.transaction().unwrap();
        let rows: Vec<Conta> = tx
            .query(ACCOUNT_SQL_FOR_UPDATE,&[&id])
            .unwrap()
            .iter()
            .map(|row| Conta {
                saldo: row.get(0),
                limite: row.get(1),
            })
            .collect();
        if rows.is_empty() {
            return Err(404);
        }
        let conta = rows.get(0).unwrap();
        let saldo = conta.saldo;
        let limite = conta.limite;
        let saldo = saldo + valor;
        if (saldo + limite) < 0 {
            return Err(422);
        }
        tx.execute("UPDATE accounts SET balance=$1 WHERE id=$2", &[&saldo, &id])
            .unwrap();
        tx.execute("INSERT INTO transactions (account_id, amount, transaction_type, details) VALUES ($1,$2,$3,$4)", 
            &[&id, &payload.valor, &payload.tipo, &descricao]).unwrap();
        tx.commit().unwrap();
        Ok(Conta { saldo, limite })
    })
    .await
    .map(|conta| ntex::web::HttpResponse::Ok().json(&conta))
    .map_err(|err| {
        let err_str = format!("{}", err);
        match err_str.as_str() {
            "404" => web::error::ErrorNotFound(err),
            _ => web::error::ErrorUnprocessableEntity(err),
 
        }
    });
    Ok(res?)
}

pub fn handler() -> ntex::web::Scope<DefaultError> {
    web::scope("/clientes")
        .service(get_balance)
        .service(post_transaction)
}
