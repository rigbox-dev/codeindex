package auth

// User represents a user in the system.
type User struct {
	ID       int
	Username string
	Email    string
}

// UserRepository manages user storage.
type UserRepository struct {
	users map[int]*User
}

// NewUserRepository creates a new UserRepository.
func NewUserRepository() *UserRepository {
	return &UserRepository{users: make(map[int]*User)}
}

// FindByID retrieves a user by ID.
func (r *UserRepository) FindByID(id int) *User {
	return r.users[id]
}

// FindByUsername retrieves a user by username.
func (r *UserRepository) FindByUsername(username string) *User {
	for _, user := range r.users {
		if user.Username == username {
			return user
		}
	}
	return nil
}

// Save stores a user.
func (r *UserRepository) Save(user *User) {
	r.users[user.ID] = user
}

// Delete removes a user by ID.
func (r *UserRepository) Delete(id int) bool {
	if _, ok := r.users[id]; ok {
		delete(r.users, id)
		return true
	}
	return false
}
