use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub token_expiry_hours: u64,
    pub max_failed_attempts: u32,
    pub lockout_duration_minutes: u32,
    pub require_https: bool,
    pub allowed_origins: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt_secret: Uuid::new_v4().to_string(),
            token_expiry_hours: 24,
            max_failed_attempts: 5,
            lockout_duration_minutes: 30,
            require_https: false,
            allowed_origins: vec!["http://localhost:8080".to_string()],
        }
    }
}

/// User account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: SystemTime,
    pub last_login: Option<SystemTime>,
    pub failed_attempts: u32,
    pub locked_until: Option<SystemTime>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    Operator,
    ReadOnly,
}

impl UserRole {
    pub fn can_read(&self) -> bool {
        matches!(
            self,
            UserRole::Admin | UserRole::Operator | UserRole::ReadOnly
        )
    }

    pub fn can_write(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Operator)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // Subject (username)
    pub role: UserRole, // User role
    pub exp: usize,     // Expiration time
    pub iat: usize,     // Issued at
    pub jti: String,    // JWT ID
}

/// Login request
#[derive(Debug, Deserialize, Clone)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub expires_in: Option<u64>,
    pub user: Option<UserInfo>,
    pub message: String,
}

/// Public user information
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub username: String,
    pub role: UserRole,
    pub last_login: Option<SystemTime>,
}

/// Authentication manager
pub struct AuthManager {
    config: AuthConfig,
    users: HashMap<String, User>,
    active_tokens: HashMap<String, Claims>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl AuthManager {
    pub fn new(config: AuthConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_bytes());

        let mut manager = Self {
            config,
            users: HashMap::new(),
            active_tokens: HashMap::new(),
            encoding_key,
            decoding_key,
        };

        // Create default admin user if none exists
        manager.create_default_admin()?;

        Ok(manager)
    }

    fn create_default_admin(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.users.is_empty() {
            let password_hash = hash("admin123", DEFAULT_COST)?;
            let user = User {
                username: "admin".to_string(),
                password_hash,
                role: UserRole::Admin,
                created_at: SystemTime::now(),
                last_login: None,
                failed_attempts: 0,
                locked_until: None,
                active: true,
            };

            self.users.insert("admin".to_string(), user);
            log::warn!("⚠️  Default admin user created with password 'admin123'. Please change immediately!");
        }
        Ok(())
    }

    pub async fn authenticate(&mut self, request: LoginRequest) -> LoginResponse {
        if !self.config.enabled {
            return LoginResponse {
                success: false,
                token: None,
                expires_in: None,
                user: None,
                message: "Authentication is disabled".to_string(),
            };
        }

        let user = match self.users.get_mut(&request.username) {
            Some(user) => user,
            None => {
                log::warn!("Login attempt for non-existent user: {}", request.username);
                return LoginResponse {
                    success: false,
                    token: None,
                    expires_in: None,
                    user: None,
                    message: "Invalid credentials".to_string(),
                };
            }
        };

        // Check if user is locked out
        if let Some(locked_until) = user.locked_until {
            if SystemTime::now() < locked_until {
                return LoginResponse {
                    success: false,
                    token: None,
                    expires_in: None,
                    user: None,
                    message: "Account is temporarily locked".to_string(),
                };
            } else {
                // Unlock the account
                user.locked_until = None;
                user.failed_attempts = 0;
            }
        }

        // Check if user is active
        if !user.active {
            return LoginResponse {
                success: false,
                token: None,
                expires_in: None,
                user: None,
                message: "Account is disabled".to_string(),
            };
        }

        // Verify password
        match verify(&request.password, &user.password_hash) {
            Ok(true) => {
                let (username, role, last_login) = {
                    user.failed_attempts = 0;
                    user.last_login = Some(SystemTime::now());
                    (user.username.clone(), user.role.clone(), user.last_login)
                };

                match self.generate_token(&username, &role) {
                    Ok(token) => {
                        log::info!("Successful login for user: {}", username);
                        LoginResponse {
                            success: true,
                            token: Some(token),
                            expires_in: Some(self.config.token_expiry_hours * 3600),
                            user: Some(UserInfo {
                                username,
                                role,
                                last_login,
                            }),
                            message: "Login successful".to_string(),
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to generate token: {}", e);
                        LoginResponse {
                            success: false,
                            token: None,
                            expires_in: None,
                            user: None,
                            message: "Internal error".to_string(),
                        }
                    }
                }
            }
            Ok(false) | Err(_) => {
                // Increment failed attempts
                user.failed_attempts += 1;
                log::warn!(
                    "Failed login attempt for user: {} (attempt {})",
                    request.username,
                    user.failed_attempts
                );

                // Lock account if too many failed attempts
                if user.failed_attempts >= self.config.max_failed_attempts {
                    let lockout_duration = std::time::Duration::from_secs(
                        self.config.lockout_duration_minutes as u64 * 60,
                    );
                    user.locked_until = Some(SystemTime::now() + lockout_duration);
                    log::warn!(
                        "Account locked for user: {} due to too many failed attempts",
                        request.username
                    );
                }

                LoginResponse {
                    success: false,
                    token: None,
                    expires_in: None,
                    user: None,
                    message: "Invalid credentials".to_string(),
                }
            }
        }
    }

    fn generate_token(
        &mut self,
        username: &str,
        role: &UserRole,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let exp = now + (self.config.token_expiry_hours as usize * 3600);
        let jti = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: username.to_string(),
            role: role.clone(),
            exp,
            iat: now,
            jti: jti.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;

        // Store active token
        self.active_tokens.insert(jti, claims);

        Ok(token)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        if !self.config.enabled {
            return Err(AuthError::Disabled);
        }

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        match decode::<Claims>(token, &self.decoding_key, &validation) {
            Ok(token_data) => {
                let claims = token_data.claims;

                // Check if token is in active tokens list
                if !self.active_tokens.contains_key(&claims.jti) {
                    return Err(AuthError::TokenRevoked);
                }

                // Check if user still exists and is active
                if let Some(user) = self.users.get(&claims.sub) {
                    if !user.active {
                        return Err(AuthError::UserDisabled);
                    }
                } else {
                    return Err(AuthError::UserNotFound);
                }

                Ok(claims)
            }
            Err(e) => {
                log::debug!("Token validation failed: {}", e);
                Err(AuthError::InvalidToken)
            }
        }
    }

    pub fn logout(&mut self, token: &str) -> Result<(), AuthError> {
        let claims = self.validate_token(token)?;
        self.active_tokens.remove(&claims.jti);
        log::info!("User {} logged out", claims.sub);
        Ok(())
    }

    pub fn create_user(
        &mut self,
        username: String,
        password: String,
        role: UserRole,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.users.contains_key(&username) {
            return Err("User already exists".into());
        }

        let password_hash = hash(&password, DEFAULT_COST)?;
        let user = User {
            username: username.clone(),
            password_hash,
            role,
            created_at: SystemTime::now(),
            last_login: None,
            failed_attempts: 0,
            locked_until: None,
            active: true,
        };

        self.users.insert(username.clone(), user);
        log::info!("Created new user: {}", username);
        Ok(())
    }

    pub fn change_password(
        &mut self,
        username: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user = self.users.get_mut(username).ok_or("User not found")?;

        if !verify(old_password, &user.password_hash)? {
            return Err("Invalid current password".into());
        }

        user.password_hash = hash(new_password, DEFAULT_COST)?;
        log::info!("Password changed for user: {}", username);
        Ok(())
    }

    pub fn deactivate_user(
        &mut self,
        username: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user = self.users.get_mut(username).ok_or("User not found")?;

        user.active = false;

        // Revoke all active tokens for this user
        self.active_tokens
            .retain(|_, claims| claims.sub != username);

        log::info!("Deactivated user: {}", username);
        Ok(())
    }

    pub fn list_users(&self) -> Vec<UserInfo> {
        self.users
            .values()
            .map(|user| UserInfo {
                username: user.username.clone(),
                role: user.role.clone(),
                last_login: user.last_login,
            })
            .collect()
    }

    pub fn cleanup_expired_tokens(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        self.active_tokens.retain(|_, claims| claims.exp > now);
    }
}

/// Authentication errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Authentication is disabled")]
    Disabled,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token has been revoked")]
    TokenRevoked,
    #[error("User not found")]
    UserNotFound,
    #[error("User account is disabled")]
    UserDisabled,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
}

/// Permission checking middleware
pub fn require_permission(required_role: UserRole) -> impl Fn(&Claims) -> Result<(), AuthError> {
    move |claims: &Claims| match required_role {
        UserRole::ReadOnly => {
            if claims.role.can_read() {
                Ok(())
            } else {
                Err(AuthError::InsufficientPermissions)
            }
        }
        UserRole::Operator => {
            if claims.role.can_write() {
                Ok(())
            } else {
                Err(AuthError::InsufficientPermissions)
            }
        }
        UserRole::Admin => {
            if claims.role.can_admin() {
                Ok(())
            } else {
                Err(AuthError::InsufficientPermissions)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_manager_creation() {
        let config = AuthConfig::default();
        let auth_manager = AuthManager::new(config).unwrap();

        // Should have default admin user
        assert_eq!(auth_manager.users.len(), 1);
        assert!(auth_manager.users.contains_key("admin"));
    }

    #[tokio::test]
    async fn test_successful_authentication() {
        let config = AuthConfig::default();
        let mut auth_manager = AuthManager::new(config).unwrap();

        let request = LoginRequest {
            username: "admin".to_string(),
            password: "admin123".to_string(),
        };

        let response = auth_manager.authenticate(request).await;
        assert!(response.success);
        assert!(response.token.is_some());
    }

    #[tokio::test]
    async fn test_failed_authentication() {
        let config = AuthConfig::default();
        let mut auth_manager = AuthManager::new(config).unwrap();

        let request = LoginRequest {
            username: "admin".to_string(),
            password: "wrong_password".to_string(),
        };

        let response = auth_manager.authenticate(request).await;
        assert!(!response.success);
        assert!(response.token.is_none());
    }

    #[test]
    fn test_user_roles() {
        assert!(UserRole::Admin.can_admin());
        assert!(UserRole::Admin.can_write());
        assert!(UserRole::Admin.can_read());

        assert!(!UserRole::Operator.can_admin());
        assert!(UserRole::Operator.can_write());
        assert!(UserRole::Operator.can_read());

        assert!(!UserRole::ReadOnly.can_admin());
        assert!(!UserRole::ReadOnly.can_write());
        assert!(UserRole::ReadOnly.can_read());
    }

    #[tokio::test]
    async fn test_account_lockout() {
        let mut config = AuthConfig::default();
        config.max_failed_attempts = 2;
        let mut auth_manager = AuthManager::new(config).unwrap();

        let request = LoginRequest {
            username: "admin".to_string(),
            password: "wrong_password".to_string(),
        };

        // First failed attempt
        let response1 = auth_manager.authenticate(request.clone()).await;
        assert!(!response1.success);

        // Second failed attempt - should lock account
        let response2 = auth_manager.authenticate(request.clone()).await;
        assert!(!response2.success);

        // Third attempt should be rejected due to lockout
        let response3 = auth_manager.authenticate(request).await;
        assert!(!response3.success);
        assert_eq!(response3.message, "Account is temporarily locked");
    }
}
