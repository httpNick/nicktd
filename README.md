# Nick's Tower Defense

This is a tower defense game being developed.

## Development

This project is set up to run in a containerized environment using Docker and Docker Compose.

### Prerequisites

- **Windows Subsystem for Linux (WSL) 2:** This project is intended to be developed within a WSL 2 environment.
- **Docker Desktop for Windows:** You must have Docker Desktop installed on your Windows machine.

### Setting up Docker Desktop for WSL

1.  **Install Docker Desktop:** Download and install Docker Desktop for Windows from the [official Docker website](https://www.docker.com/products/docker-desktop).
2.  **Enable WSL 2 Integration:**
    -   Open Docker Desktop.
    -   Go to **Settings** > **Resources** > **WSL Integration**.
    -   Ensure the toggle is **On** for your WSL distribution (e.g., "Ubuntu").

### Running the Application (Containerized)

Once Docker Desktop is set up, you can run the application from your WSL terminal:

1.  **Navigate to the project directory:**
    ```bash
    cd /path/to/your/project/nicktd
    ```
2.  **Build and run the container in detached mode:**
    ```bash
    docker-compose up --build -d
    ```

3.  **Stopping the container:**
    ```bash
    docker-compose down
    ```

4.  **Accessing the server:**
    -   The server will be running inside the container and accessible from your host machine (Windows) at `ws://localhost:9001` or `ws://127.0.0.1:9001`. Your frontend application can connect to this address.

5.  **Viewing container logs:**
    To view the live output of the server running in the container:
    ```bash
    docker-compose logs -f
    ```

### Database (SQLite)

The application uses SQLite for its database.

-   The database file (`nicktd.db`) is stored in the `data/` directory at the root of this project on your host machine. This directory is mounted into the container, ensuring data persistence across container restarts.
-   **To inspect the database:**
    1.  Ensure the container is running (`docker-compose up -d`).
    2.  You can use a local SQLite browser (e.g., DB Browser for SQLite) and open `data/nicktd.db` from your project root.
    3.  Alternatively, you can temporarily install `sqlite3` inside your container for inspection:
        ```bash
        # First, exec into the container and install sqlite3
        docker-compose exec nicktd apt-get update && docker-compose exec nicktd apt-get install -y sqlite3
        # Then, connect to the database
        docker-compose exec nicktd sqlite3 /usr/src/app/data/nicktd.db
        # Inside the sqlite3 prompt, you can run SQL commands (e.g., .tables, SELECT * FROM my_table;)
        ```

### Running the Application (Local Development)

It is also possible to run the server directly on your host machine for faster iteration during development.

1.  **Prerequisites for Local Rust Development:**
    -   **Rust Toolchain:** Install Rust via `rustup`.

2.  **Stop Docker Containers (if running):**
    ```bash
    docker-compose down
    ```

3.  **Navigate to the server directory:**
    ```bash
    cd server
    ```

4.  **Run the server:**
    ```bash
    cargo run
    ```
    -   When running locally, your `database.rs` will create and manage a `nicktd.db` file within a `data/` directory inside your `server/` folder (`server/data/nicktd.db`). This is separate from the Docker-managed database.

### Verifying the Setup

You can verify that Docker is correctly integrated with your WSL environment by running the following commands in your WSL terminal:

-   `docker --version`
-   `docker-compose --version`
-   `docker ps`

## Frontend Development

The frontend is a TypeScript application located in the `view/` directory. It uses [Vite](https://vitejs.dev/) for development and building.

### Prerequisites

- **Node.js:** Ensure you have Node.js installed (v18+ recommended).

### Setup

1. **Navigate to the view directory:**

   ```bash
   cd view
   ```

2. **Install dependencies:**

   ```bash
   npm install
   ```

### Running the Frontend

To start the development server with hot-module replacement (HMR):

```bash
cd view
npm run dev
```

By default, the application will be available at `http://localhost:5173`. It expects the backend server to be running at `http://localhost:9001`.

### Testing & Quality
- **Type Checking:** Run `npm run type-check` to validate TypeScript types without building.
- **Production Build:** Run `npm run build` to generate a production-ready bundle in `view/dist/`.
- **Unit Tests:** Run `npm test` to execute frontend logic tests.