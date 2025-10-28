from flask import Flask
import psycopg2

import os

app = Flask(__name__)

@app.route('/')
def hello_world():
    return 'Hello World'

if __name__ == '__main__':
    database = retrieve_env_var("DB_NAME")
    user = retrieve_env_var("DB_USER")
    password = retrieve_env_var("DB_PASSWORD")
    host = retrieve_env_var("DB_ENDPOINT")

    conn = psycopg2.connect(database=database, user=user,
                            password=password, host=host, port="5432")
    cur = conn.cursor()

    app.run()

def retrieve_env_var(var: str) -> str:
    try:
        return os.environ[key]
    except KeyError:
        raise RuntimeError(f"Missing required environment variable: {key}")
