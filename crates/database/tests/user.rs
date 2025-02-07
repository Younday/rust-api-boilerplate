use sqlx::{PgPool};
use database::{user::repository::UserRepositoryTrait, Database};

#[sqlx::test]
async fn create_user(pool: PgPool) -> sqlx::Result<()> {
    let database = Database{ db: pool };

    let user = database.create_user("name", "email@email.com", "password123").await.unwrap();

    assert_eq!(user.name, "name");
    assert_eq!(user.email, "email@email.com");
    assert_eq!(user.password, "password123");
    
    Ok(())
}

#[sqlx::test]
async fn get_user_by_email(pool: PgPool) -> sqlx::Result<()> {
    let database = Database{ db: pool };
    let user = database.create_user("name", "email@email.com", "password123").await.unwrap();
    let user_by_email = database.get_user_by_email("email@email.com").await.unwrap();

    assert_eq!(user.email, user_by_email.email);

    Ok(())
}

#[sqlx::test]
async fn get_user_by_id(pool: PgPool) -> sqlx::Result<()> {
    let database = Database{ db: pool };
    let user = database.create_user("name", "email@email.com", "password123").await.unwrap();
    let user_by_id = database.get_user_by_id(user.id).await.unwrap();

    assert_eq!(user.id, user_by_id.id);

    Ok(())
}