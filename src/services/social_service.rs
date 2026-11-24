use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{Follow, Post, User},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowerItem {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub followed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedPost {
    pub id: String,
    pub author_id: String,
    pub author: Option<PublicUser>,
    pub content: String,
    pub symbol: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub is_hidden: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostEngagement {
    pub post_id: String,
    pub likes_count: i64,
    pub liked_by_viewer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialPagination {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedResponse {
    pub posts: Vec<SerializedPost>,
    pub pagination: SocialPagination,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostPayload {
    pub content: String,
    pub symbol: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

pub struct SocialService;

impl SocialService {
    async fn ensure_active_user(pool: &PgPool, user_id: &str) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("User not found.", "USER_NOT_FOUND"))?;

        if user.is_suspended {
            return Err(AppError::forbidden(
                "Account is suspended.",
                "ACCOUNT_SUSPENDED",
            ));
        }

        Ok(user)
    }

    pub async fn follow_user(
        pool: &PgPool,
        user_id: &str,
        target_user_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        if user_id == target_user_id {
            return Err(AppError::bad_request(
                "You cannot follow yourself.",
                "CANNOT_FOLLOW_SELF",
            ));
        }

        let target = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(target_user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("User not found.", "USER_NOT_FOUND"))?;

        if target.is_suspended {
            return Err(AppError::bad_request("User is suspended.", "USER_SUSPENDED"));
        }

        let follow_id = Uuid::new_v4().to_string();
        let follow = sqlx::query_as::<_, Follow>(
            r#"
            INSERT INTO "Follow" ("id", "followerId", "followingId", "createdAt")
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT ("followerId", "followingId") DO NOTHING
            RETURNING *
            "#,
        )
        .bind(&follow_id)
        .bind(user_id)
        .bind(target_user_id)
        .fetch_optional(pool)
        .await?;

        let follow = match follow {
            Some(f) => f,
            None => {
                return Err(AppError::conflict(
                    "You already follow this user.",
                    "ALREADY_FOLLOWING",
                ));
            }
        };

        // Create notification for target user
        let notif_id = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            r#"
            INSERT INTO "Notification" (
                "id", "userId", "type", "title", "message", "status", "data", "createdAt"
            )
            VALUES ($1, $2, 'NEW_FOLLOWER', 'New follower', 'Someone started following you.', 'PENDING', $3, NOW())
            "#,
        )
        .bind(&notif_id)
        .bind(target_user_id)
        .bind(serde_json::json!({ "followerId": user_id }))
        .execute(pool)
        .await;

        Ok(serde_json::json!({
            "followerId": follow.follower_id,
            "followingId": follow.following_id,
            "createdAt": follow.created_at,
        }))
    }

    pub async fn unfollow_user(
        pool: &PgPool,
        user_id: &str,
        target_user_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let res = sqlx::query(
            r#"DELETE FROM "Follow" WHERE "followerId" = $1 AND "followingId" = $2"#,
        )
        .bind(user_id)
        .bind(target_user_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::not_found(
                "You do not follow this user.",
                "NOT_FOLLOWING",
            ));
        }

        Ok(serde_json::json!({
            "followingId": target_user_id,
            "unfollowed": true
        }))
    }

    pub async fn list_followers(
        pool: &PgPool,
        user_id: &str,
    ) -> Result<Vec<FollowerItem>, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let rows = sqlx::query_as::<_, (String, String, Option<String>, DateTime<Utc>)>(
            r#"
            SELECT u."id", u."name", u."avatarUrl", f."createdAt"
            FROM "Follow" f
            JOIN "User" u ON f."followerId" = u."id"
            WHERE f."followingId" = $1
            ORDER BY f."createdAt" DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let items = rows
            .into_iter()
            .map(|(id, name, avatar_url, followed_at)| FollowerItem {
                id,
                name,
                avatar_url,
                followed_at,
            })
            .collect();

        Ok(items)
    }

    pub async fn list_following(
        pool: &PgPool,
        user_id: &str,
    ) -> Result<Vec<FollowerItem>, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let rows = sqlx::query_as::<_, (String, String, Option<String>, DateTime<Utc>)>(
            r#"
            SELECT u."id", u."name", u."avatarUrl", f."createdAt"
            FROM "Follow" f
            JOIN "User" u ON f."followingId" = u."id"
            WHERE f."followerId" = $1
            ORDER BY f."createdAt" DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let items = rows
            .into_iter()
            .map(|(id, name, avatar_url, followed_at)| FollowerItem {
                id,
                name,
                avatar_url,
                followed_at,
            })
            .collect();

        Ok(items)
    }

    pub async fn create_post(
        pool: &PgPool,
        user_id: &str,
        payload: CreatePostPayload,
    ) -> Result<SerializedPost, AppError> {
        let user = Self::ensure_active_user(pool, user_id).await?;

        let content = payload.content.trim();
        if content.is_empty() {
            return Err(AppError::bad_request("Content is required.", "VALIDATION_ERROR"));
        }

        let post_id = Uuid::new_v4().to_string();
        let post = sqlx::query_as::<_, Post>(
            r#"
            INSERT INTO "Post" ("id", "authorId", "content", "symbol", "metadata", "isHidden", "createdAt", "updatedAt")
            VALUES ($1, $2, $3, $4, $5, false, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(&post_id)
        .bind(user_id)
        .bind(content)
        .bind(payload.symbol)
        .bind(payload.metadata)
        .fetch_one(pool)
        .await?;

        Ok(SerializedPost {
            id: post.id,
            author_id: post.author_id,
            author: Some(PublicUser {
                id: user.id,
                name: user.name,
                avatar_url: user.avatar_url,
            }),
            content: post.content,
            symbol: post.symbol,
            metadata: post.metadata,
            is_hidden: post.is_hidden,
            created_at: post.created_at,
            updated_at: post.updated_at,
        })
    }

    pub async fn delete_post(
        pool: &PgPool,
        user_id: &str,
        post_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let res = sqlx::query(
            r#"DELETE FROM "Post" WHERE "id" = $1 AND "authorId" = $2"#,
        )
        .bind(post_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::not_found("Post not found.", "POST_NOT_FOUND"));
        }

        Ok(serde_json::json!({
            "postId": post_id,
            "deleted": true
        }))
    }

    pub async fn get_post_by_id(
        pool: &PgPool,
        post_id: &str,
    ) -> Result<SerializedPost, AppError> {
        let post = sqlx::query_as::<_, Post>(
            r#"SELECT * FROM "Post" WHERE "id" = $1 AND "isHidden" = false"#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Post not found.", "POST_NOT_FOUND"))?;

        let author = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(&post.author_id)
            .fetch_optional(pool)
            .await?;

        Ok(SerializedPost {
            id: post.id,
            author_id: post.author_id,
            author: author.map(|u| PublicUser {
                id: u.id,
                name: u.name,
                avatar_url: u.avatar_url,
            }),
            content: post.content,
            symbol: post.symbol,
            metadata: post.metadata,
            is_hidden: post.is_hidden,
            created_at: post.created_at,
            updated_at: post.updated_at,
        })
    }

    pub async fn get_post_engagement(
        pool: &PgPool,
        user_id: &str,
        post_id: &str,
    ) -> Result<PostEngagement, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let post_exists: Option<(String,)> = sqlx::query_as(
            r#"SELECT "id" FROM "Post" WHERE "id" = $1 AND "isHidden" = false"#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await?;

        if post_exists.is_none() {
            return Err(AppError::not_found("Post not found.", "POST_NOT_FOUND"));
        }

        let likes_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "PostLike" WHERE "postId" = $1"#,
        )
        .bind(post_id)
        .fetch_one(pool)
        .await?;

        let liked_by_viewer: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "PostLike" WHERE "postId" = $1 AND "userId" = $2"#,
        )
        .bind(post_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(PostEngagement {
            post_id: post_id.to_string(),
            likes_count: likes_count.0,
            liked_by_viewer: liked_by_viewer.0 > 0,
        })
    }

    pub async fn like_post(
        pool: &PgPool,
        user_id: &str,
        post_id: &str,
    ) -> Result<PostEngagement, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let post = sqlx::query_as::<_, Post>(
            r#"SELECT * FROM "Post" WHERE "id" = $1 AND "isHidden" = false"#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Post not found.", "POST_NOT_FOUND"))?;

        let existing: Option<(String,)> = sqlx::query_as(
            r#"SELECT "id" FROM "PostLike" WHERE "postId" = $1 AND "userId" = $2"#,
        )
        .bind(post_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            return Err(AppError::conflict("You already liked this post.", "ALREADY_LIKED"));
        }

        let like_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO "PostLike" ("id", "postId", "userId", "createdAt") VALUES ($1, $2, $3, NOW())"#,
        )
        .bind(&like_id)
        .bind(post_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        let likes_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "PostLike" WHERE "postId" = $1"#,
        )
        .bind(post_id)
        .fetch_one(pool)
        .await?;

        Ok(PostEngagement {
            post_id: post.id,
            likes_count: likes_count.0,
            liked_by_viewer: true,
        })
    }

    pub async fn unlike_post(
        pool: &PgPool,
        user_id: &str,
        post_id: &str,
    ) -> Result<PostEngagement, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let post = sqlx::query_as::<_, Post>(
            r#"SELECT * FROM "Post" WHERE "id" = $1 AND "isHidden" = false"#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Post not found.", "POST_NOT_FOUND"))?;

        let res = sqlx::query(
            r#"DELETE FROM "PostLike" WHERE "postId" = $1 AND "userId" = $2"#,
        )
        .bind(post_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::not_found("You have not liked this post.", "POST_NOT_LIKED"));
        }

        let likes_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "PostLike" WHERE "postId" = $1"#,
        )
        .bind(post_id)
        .fetch_one(pool)
        .await?;

        Ok(PostEngagement {
            post_id: post.id,
            likes_count: likes_count.0,
            liked_by_viewer: false,
        })
    }

    pub async fn get_feed(
        pool: &PgPool,
        viewer_id: Option<&str>,
        page: i64,
        limit: i64,
        sort: &str,
    ) -> Result<FeedResponse, AppError> {
        let offset = (page - 1) * limit;

        if sort.eq_ignore_ascii_case("trending") {
            let now = Utc::now();
            let seven_days_ago = now - chrono::Duration::days(7);

            let candidates = sqlx::query_as::<_, Post>(
                r#"
                SELECT p.* FROM "Post" p
                JOIN "User" u ON p."authorId" = u."id"
                WHERE p."isHidden" = false
                  AND u."isSuspended" = false
                  AND p."createdAt" >= $1
                  AND p."createdAt" <= $2
                ORDER BY p."createdAt" DESC
                LIMIT 200
                "#,
            )
            .bind(seven_days_ago)
            .bind(now)
            .fetch_all(pool)
            .await?;

            let mut following_set = HashSet::new();
            if let Some(vid) = viewer_id {
                let author_ids: Vec<String> = candidates.iter().map(|p| p.author_id.clone()).collect();
                let follows = sqlx::query_as::<_, (String,)>(
                    r#"SELECT "followingId" FROM "Follow" WHERE "followerId" = $1 AND "followingId" = ANY($2)"#,
                )
                .bind(vid)
                .bind(&author_ids)
                .fetch_all(pool)
                .await?;

                for (f_id,) in follows {
                    following_set.insert(f_id);
                }
            }

            let mut scored: Vec<(Post, f64)> = Vec::new();
            for post in candidates {
                let likes_count: (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM "PostLike" WHERE "postId" = $1"#,
                )
                .bind(&post.id)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

                let is_following = viewer_id.map_or(false, |_| following_set.contains(&post.author_id));
                let age_hours = (now - post.created_at).num_seconds() as f64 / 3600.0;
                let numerator = (likes_count.0 as f64) + 1.0 + (if is_following { 2.0 } else { 0.0 });
                let denominator = (age_hours + 2.0).powf(1.2);
                let score = numerator / denominator;

                scored.push((post, score));
            }

            scored.sort_by(|a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.0.created_at.cmp(&a.0.created_at))
            });

            let total = scored.len() as i64;
            let start = offset as usize;
            let paged_posts = if start < scored.len() {
                scored[start..].iter().take(limit as usize).map(|(p, _)| p.clone()).collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            let mut serialized = Vec::new();
            for post in paged_posts {
                let author = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
                    .bind(&post.author_id)
                    .fetch_optional(pool)
                    .await?;

                serialized.push(SerializedPost {
                    id: post.id,
                    author_id: post.author_id,
                    author: author.map(|u| PublicUser {
                        id: u.id,
                        name: u.name,
                        avatar_url: u.avatar_url,
                    }),
                    content: post.content,
                    symbol: post.symbol,
                    metadata: post.metadata,
                    is_hidden: post.is_hidden,
                    created_at: post.created_at,
                    updated_at: post.updated_at,
                });
            }

            let total_pages = if limit > 0 { (total + limit - 1) / limit } else { 1 };
            Ok(FeedResponse {
                posts: serialized,
                pagination: SocialPagination {
                    page,
                    limit,
                    total,
                    total_pages,
                },
            })
        } else {
            let posts = sqlx::query_as::<_, Post>(
                r#"
                SELECT p.* FROM "Post" p
                JOIN "User" u ON p."authorId" = u."id"
                WHERE p."isHidden" = false AND u."isSuspended" = false
                ORDER BY p."createdAt" DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

            let total_row: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(*) FROM "Post" p
                JOIN "User" u ON p."authorId" = u."id"
                WHERE p."isHidden" = false AND u."isSuspended" = false
                "#,
            )
            .fetch_one(pool)
            .await?;

            let mut serialized = Vec::new();
            for post in posts {
                let author = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
                    .bind(&post.author_id)
                    .fetch_optional(pool)
                    .await?;

                serialized.push(SerializedPost {
                    id: post.id,
                    author_id: post.author_id,
                    author: author.map(|u| PublicUser {
                        id: u.id,
                        name: u.name,
                        avatar_url: u.avatar_url,
                    }),
                    content: post.content,
                    symbol: post.symbol,
                    metadata: post.metadata,
                    is_hidden: post.is_hidden,
                    created_at: post.created_at,
                    updated_at: post.updated_at,
                });
            }

            let total = total_row.0;
            let total_pages = if limit > 0 { (total + limit - 1) / limit } else { 1 };
            Ok(FeedResponse {
                posts: serialized,
                pagination: SocialPagination {
                    page,
                    limit,
                    total,
                    total_pages,
                },
            })
        }
    }
}
