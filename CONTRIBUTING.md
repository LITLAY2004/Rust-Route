# 🤝 Contributing to RustRoute

Thank you for your interest in contributing to RustRoute! This document provides guidelines and information for contributors.

## 🌟 Ways to Contribute

- 🐛 **Bug Reports**: Report issues you encounter
- 💡 **Feature Requests**: Suggest new features or improvements
- 🔧 **Code Contributions**: Submit pull requests with fixes or enhancements
- 📖 **Documentation**: Improve docs, examples, and tutorials
- 🧪 **Testing**: Add test cases and improve test coverage

## 🚀 Getting Started

### Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: Version control system
- **Python 3.8+**: For demo scripts (optional)

### Setup Development Environment

```bash
# Clone the repository
git clone https://github.com/LITLAY2004/Rust-Route.git
cd Rust-Route

# Build the project
cargo build

# Run tests
cargo test

# Run the demo
cargo run --example basic_router
```

## 📋 Development Guidelines

### Code Style

We follow Rust's official style guidelines:

```bash
# Format code
cargo fmt

# Lint code
cargo clippy --all-targets --all-features -- -D warnings

# Check all
cargo fmt && cargo clippy && cargo test
```

### Commit Messages

Use conventional commit format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(router): add support for IPv6 routes
fix(protocol): handle malformed RIP packets gracefully
docs(readme): update installation instructions
```

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring

## 🧪 Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run doctests
cargo test --doc
```

### Writing Tests

- **Unit tests**: Test individual functions and modules
- **Integration tests**: Test component interactions
- **Property tests**: Use `proptest` for property-based testing

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_parsing() {
        let route = Route::new("192.168.1.0", 24, "192.168.1.1", 1);
        assert_eq!(route.network(), "192.168.1.0");
        assert_eq!(route.prefix_length(), 24);
    }
}
```

## 📖 Documentation

### Code Documentation

- Use `///` for public API documentation
- Include examples in documentation
- Document all public functions, structs, and modules

```rust
/// Represents a network route in the routing table.
/// 
/// # Examples
/// 
/// ```
/// use rust_route::Route;
/// 
/// let route = Route::new("192.168.1.0", 24, "192.168.1.1", 1);
/// assert_eq!(route.network(), "192.168.1.0");
/// ```
pub struct Route {
    // ...
}
```

### README and Guides

- Keep README.md up to date
- Update USER_MANUAL.md for new features
- Add examples for new functionality

## 🔍 Code Review Process

### Submitting Pull Requests

1. **Fork** the repository
2. **Create** a feature branch
3. **Make** your changes
4. **Test** thoroughly
5. **Document** your changes
6. **Submit** a pull request

### Pull Request Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement

## Testing
- [ ] Tests pass locally
- [ ] Added new tests
- [ ] Updated documentation

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No new warnings
```

### Review Criteria

- ✅ **Functionality**: Does it work as intended?
- ✅ **Code Quality**: Is it well-structured and readable?
- ✅ **Performance**: Any performance implications?
- ✅ **Testing**: Adequate test coverage?
- ✅ **Documentation**: Proper documentation?
- ✅ **Security**: No security vulnerabilities?

## 🐛 Reporting Issues

### Bug Reports

Include the following information:

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Run command '...'
2. Send packet '...'
3. See error

**Expected behavior**
What you expected to happen.

**Environment:**
- OS: [e.g. Ubuntu 20.04]
- Rust version: [e.g. 1.70.0]
- RustRoute version: [e.g. 0.2.0]

**Additional context**
Any other context about the problem.
```

### Feature Requests

```markdown
**Is your feature request related to a problem?**
A clear description of what the problem is.

**Describe the solution you'd like**
A clear description of what you want to happen.

**Describe alternatives you've considered**
Alternative solutions or features you've considered.

**Additional context**
Any other context about the feature request.
```

## 🏗️ Architecture Overview

### Project Structure

```
rust-route/
├── src/
│   ├── main.rs           # Main entry point
│   ├── router.rs         # Core router implementation
│   ├── routing_table.rs  # Routing table management
│   ├── protocol.rs       # RIP protocol implementation
│   └── lib.rs           # Library interface
├── examples/            # Usage examples
├── tests/              # Integration tests
├── benches/            # Performance benchmarks
└── docs/               # Additional documentation
```

### Key Components

- **Router**: Main router logic and packet processing
- **RoutingTable**: Route storage and lookup
- **Protocol**: RIP packet parsing and generation
- **Config**: Configuration management

## 🚀 Release Process

### Version Numbering

We follow [Semantic Versioning](https://semver.org/):
- `MAJOR.MINOR.PATCH`
- `MAJOR`: Breaking changes
- `MINOR`: New features (backward compatible)
- `PATCH`: Bug fixes (backward compatible)

### Release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update `CHANGELOG.md`
- [ ] Run full test suite
- [ ] Update documentation
- [ ] Create release PR
- [ ] Tag release after merge

## 🎉 Recognition

Contributors will be:
- 🏷️ Listed in the project README
- 📝 Mentioned in release notes
- 🎖️ Added to the contributors list

## 📞 Getting Help

- 💬 **Discussions**: Use GitHub Discussions for questions
- 🐛 **Issues**: Report bugs via GitHub Issues
- 📧 **Email**: Contact maintainers directly for sensitive issues

## 📜 Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive.

## 📄 License

By contributing to RustRoute, you agree that your contributions will be licensed under the MIT License.

---

**Happy Contributing!** 🦀✨

Thank you for helping make RustRoute better for everyone!
