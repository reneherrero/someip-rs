# Contributing to someip-rs

Thank you for your interest in contributing to someip-rs! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

This project adheres to a code of conduct that all contributors are expected to follow. Please be respectful and constructive in all interactions.

## Getting Started

### Prerequisites

- Familiarity with Rust and the SOME/IP protocol
- Latest stable release of Rust (MSRV: 1.85.0)

### Setting Up the Development Environment

1. Clone the repository
2. Run `cargo build` to verify setup
3. Run `cargo test` to run the test suite

## Development Workflow

### Basic Commands

```bash
# Build
cargo check --all-targets
cargo check --all-features

# Test
cargo test
cargo test --all-features

# Format
cargo fmt

# Lint
cargo clippy --all-features -- -D warnings
```

### Testing and Quality Checks

**Quick reference:**
- **Tests**: Must pass with and without `tokio` feature
- **MSRV**: Must work with Rust 1.85.0

### Coding Standards

#### Code Style

- Follow the existing code style in the project
- Use `cargo fmt` to format your code

#### Documentation

- **All public APIs must be documented** with doc comments (`///`)
- Use code examples in documentation when helpful
- Document error conditions and return values
- Follow Rust documentation conventions

#### Error Handling

- Use `Result<T>` for fallible operations
- Use appropriate error variants (`SomeIpError::Io`, `SomeIpError::InvalidHeader`, etc.)

#### Testing

- Write tests for new functionality
- Include both positive and negative test cases
- Test edge cases and error conditions
- Ensure tests pass with and without optional features

#### Safety

- Avoid `unwrap()` and `expect()` in production code (tests are fine)
- Use proper error handling with the `Result` type

### Commit Messages

Write clear, descriptive commit messages:

```
Short summary (50 chars or less)

More detailed explanation if needed. Wrap at 72 characters. Explain:
- What changed and why
- Any breaking changes
- Related issues

Fixes #123
```

### Pull Requests

1. Update documentation if you're adding new features
2. Add tests for new functionality
3. Ensure all CI checks pass
4. Reference any related issues in your PR description

#### PR Checklist

- [ ] Code follows the project's guidelines
- [ ] All tests pass (`cargo test` and `cargo test --all-features`)
- [ ] Clippy passes without warnings
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation is updated

## Project Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed module structure, design principles, and technical documentation.

## Areas for Contribution

### High Priority

- More comprehensive test coverage
- Additional examples

### Medium Priority

- Performance optimizations
- Extended SOME/IP-SD functionality
- Connection pool improvements

### Low Priority

- Additional transport options
- Metrics and observability

## Questions?

If you have questions or need help:

- Open an issue on GitHub
- Check existing issues and discussions
- Review the documentation in the README files

## License

By contributing to someip-rs, you agree that your contributions will be licensed under the same license as the project (MIT OR Apache-2.0). See [LICENSING.md](LICENSING.md) for details.

Thank you for contributing!
