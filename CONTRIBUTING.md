# Contributing to Wikiwalk

Thank you for your interest in contributing to Wikiwalk! This document provides guidelines and setup instructions for development.

## Development Setup

### Prerequisites

- **Rust**: Install via [rustup](https://rustup.rs/)
- **Node.js 22**: Managed via [mise](https://mise.jdx.dev/) (see `.mise.toml`)
- **System dependencies**: `libssl-dev`, `pkg-config`

### Initial Setup

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd wikiwalk
   ```

2. **Install mise and set up Node.js**:
   ```bash
   # Install mise (see https://mise.jdx.dev/getting-started.html)
   curl https://mise.run | sh

   # Install Node.js version specified in .mise.toml
   mise install
   ```

3. **Install system dependencies**:
   ```bash
   # Ubuntu/Debian
   sudo apt-get update && sudo apt-get install -y libssl-dev pkg-config

   # macOS
   brew install openssl pkg-config
   ```

4. **Install UI dependencies**:
   ```bash
   cd wikiwalk-ui
   npm install
   cd ..
   ```

### Pre-commit Hooks Setup

We use pre-commit hooks to ensure code quality and run the same checks as our CI pipeline.

1. **Install pre-commit**:
   ```bash
   pip install pre-commit
   # or
   brew install pre-commit
   ```

2. **Install the git hooks**:
   ```bash
   pre-commit install
   ```

3. **Run hooks on all files** (optional, to check current state):
   ```bash
   pre-commit run --all-files
   ```

### What the Pre-commit Hooks Check

The hooks run the same checks as our GitHub CI:

- **Rust code quality**:
  - `cargo fmt` - Code formatting
  - `cargo clippy` - Linting with warnings as errors
  - `cargo test --verbose` - All tests
  - `cargo build --release --bin server` - Server build
  - `cargo build --release --bin tool` - Tool build

- **Frontend code quality**:
  - `npm run lint` - ESLint for TypeScript/React
  - `npm run build` - TypeScript compilation and Vite build

- **General**:
  - Trailing whitespace removal
  - End-of-file fixing
  - YAML syntax validation
  - Large file detection

## Project Structure

```
wikiwalk/
├── wikiwalk/           # Core library (graph algorithms, database)
├── server/             # Web server (Actix-web API)
├── tool/               # CLI tool (Wikipedia dump import)
├── migration/          # Database migrations
├── wikiwalk-ui/        # React frontend
├── .github/workflows/  # CI/CD configuration
└── target/             # Rust build artifacts
```

## Development Workflow

### Rust Development

1. **Build the project**:
   ```bash
   cargo build
   ```

2. **Run tests**:
   ```bash
   cargo test
   ```

3. **Run the server** (requires database setup):
   ```bash
   cargo run --bin server
   ```

4. **Format code**:
   ```bash
   cargo fmt
   ```

5. **Run linter**:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

### Frontend Development

1. **Start development server**:
   ```bash
   cd wikiwalk-ui
   npm run dev
   ```

2. **Build for production**:
   ```bash
   npm run build
   ```

3. **Lint code**:
   ```bash
   npm run lint
   ```

### Database Setup

The project uses Wikipedia dumps to build its graph database. See the main README.md for detailed instructions on:
- Downloading Wikipedia dumps
- Importing data using the `tool` binary
- Database schema and format

## Code Style

- **Rust**: Follow standard Rust conventions, enforced by `rustfmt` and `clippy`
- **TypeScript/React**: Follow the ESLint configuration in `wikiwalk-ui/.eslintrc.cjs`
- **Commits**: Use conventional commit format when possible

## Testing

- **Unit tests**: Include unit tests for new functionality
- **Integration tests**: Test API endpoints and core algorithms
- **Frontend tests**: Currently using ESLint for static analysis

## Pull Request Process

1. **Fork and create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** and ensure:
   - All pre-commit hooks pass
   - Tests pass locally
   - Code follows project conventions

3. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

4. **Push and create a pull request**:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Ensure CI passes** - The same checks from pre-commit hooks will run in GitHub Actions

## Performance Considerations

- **Memory usage**: The graph database uses memory-mapped files for performance
- **Build times**: Release builds are optimized but may take longer
- **Test data**: Use smaller datasets for local development when possible

## Getting Help

- **Issues**: Check existing issues or create a new one for bugs/features
- **Discussions**: Use GitHub Discussions for questions and ideas
- **Code**: Reference the main README.md for architectural details

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.