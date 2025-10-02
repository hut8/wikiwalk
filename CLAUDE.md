# Wikiwalk Project

## Overview
Wikiwalk is a Rust-based application that implements a shortest path algorithm between Wikipedia pages. It finds the set of shortest paths between any two Wikipedia pages and returns the path as a list of page titles.

## Architecture
This is a Rust workspace with multiple components:

- **wikiwalk**: Core library containing the shortest path algorithm and graph database
- **server**: Web server (Actix-web) that serves the API and UI
- **tool**: Command-line tool for database import and management
- **migration**: Database migration utilities
- **wikiwalk-ui**: React/TypeScript frontend with Vite build system

## Technology Stack

### Backend (Rust)
- **Core**: Rust 2021 edition with Cargo workspace
- **Web Framework**: Actix-web 4.8.0
- **Database**: SQLite with SeaORM 1.0.1
- **Async Runtime**: Tokio 1.43.0
- **Graph Storage**: Custom memory-mapped binary format for adjacency lists
- **Parsing**: Custom MediaWiki SQL dump parser

### Frontend (TypeScript/React)
- **Framework**: React 18.2.0 with TypeScript 5.0.2
- **Build Tool**: Vite 4.5.9
- **UI Framework**: Material-UI 5.14.11
- **State Management**: TanStack Query 4.35.3
- **Routing**: React Router 6.16.0
- **Visualization**: Cosmograph for graph rendering

### Development Tools
- **Node Version**: 22 (managed via mise)
- **Linting**: ESLint with TypeScript support
- **CI/CD**: GitHub Actions on Ubuntu 24.04

## Key Features
- Bidirectional BFS algorithm for finding shortest paths between Wikipedia pages
- Custom graph database with memory-mapped adjacency lists for performance
- Wikipedia dump import pipeline with incremental updates
- Web API for path queries
- Interactive graph visualization
- Caching and performance optimization

## Database Design
The project uses two main storage systems:
1. **SQLite databases**: For persistent data, caching, and metadata
2. **Custom graph format**: Binary adjacency lists (`vertex_al` and `vertex_al_ix`) for fast graph traversal

## Build and Test Commands
- **Build**: `cargo build --release`
- **Test**: `cargo test --verbose`
- **Lint (UI)**: `cd wikiwalk-ui && npm run lint`
- **Build (UI)**: `cd wikiwalk-ui && npm run build`

## Development Workflow
1. Import Wikipedia dumps using the `tool` binary
2. Build graph database from imported data
3. Run server for API and UI
4. Frontend development with hot reload via Vite

## Dependencies
- System: libssl-dev, pkg-config
- Runtime: Node.js 22 for UI development
- After changing rust code, before always run "cargo fmt"
