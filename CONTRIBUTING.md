# Contributing to dx

Thank you for considering contributing to dx! 🎉

## Getting Started

1. **Fork** the repository
2. **Clone** your fork locally
3. **Install** Rust (stable channel)
4. **Build** the project: `cargo build`
5. **Test** your changes: `cargo test`
6. **Lint** your code: `cargo clippy -- -D warnings`

## Development Workflow

### Local Development
```bash
# Build and install locally for testing
./install.sh

# Run tests
cargo test

# Check code quality
cargo clippy -- -D warnings
```

### Using dx to develop dx (dogfooding!)
Once you have dx installed, you can use it to build itself:
```bash
dx build:release
dx build:test
dx build:clippy
```

## Pull Request Process

1. **Create a feature branch** from `main`
2. **Make your changes** with clear, focused commits
3. **Add tests** for new functionality
4. **Update documentation** if needed
5. **Ensure all tests pass** and clippy is happy
6. **Create a Pull Request** with a clear description

## Code Style

- Follow standard Rust conventions (use `rustfmt`)
- Write clear, self-documenting code
- Add doc comments for public APIs
- Keep functions focused and testable

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Include steps to reproduce for bugs
- Provide relevant system information (OS, terminal, etc.)

## Questions?

Feel free to open an issue for questions or discussion!
