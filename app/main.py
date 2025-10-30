from flask import Flask, request
import psycopg2

import os

app = Flask(__name__)

@app.route('/')
def hello_world():
    return 'Hello World'

@app.route('/search')
def search_results():
    conn = db_conn()

    query = request.args.get('q')
    return str(query)

if __name__ == '__main__':
    app.run()

def retrieve_env_var(var: str) -> str:
    try:
        return os.environ[var]
    except KeyError:
        raise RuntimeError(f"Missing required environment variable: {var}")

# TODO: Add return type
def db_conn():
    database = retrieve_env_var("DB_NAME")
    user = retrieve_env_var("DB_USER")
    password = retrieve_env_var("DB_PASSWORD")
    host = retrieve_env_var("DB_ENDPOINT")

    conn = psycopg2.connect(database=database, user=user,
                            password=password, host=host, port="5432")
    return conn.cursor()

