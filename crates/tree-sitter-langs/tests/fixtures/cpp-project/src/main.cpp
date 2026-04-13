#include "auth.h"
#include <stdio.h>

int main() {
    struct AuthRequest req = {"admin", "secret"};
    int result = authenticate(&req);
    return 0;
}
