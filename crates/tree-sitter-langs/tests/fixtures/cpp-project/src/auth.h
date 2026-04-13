#ifndef AUTH_H
#define AUTH_H

struct AuthRequest {
    const char* username;
    const char* password;
};

enum AuthError {
    AUTH_OK = 0,
    AUTH_INVALID_CREDENTIALS,
};

int authenticate(struct AuthRequest* req);

#endif
