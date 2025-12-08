FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .

RUN pip install --no-cache-dir -r requirements.txt

# Update public suffixes for url parsing
RUN tldextract --update

# Copy app code
COPY . .

# Tell pytest where root is
ENV PYTHONPATH=/app
RUN pytest tests/ -vv

# Tell flask where to find app
ENV FLASK_APP=main

# Tell flash where to host the server
ENV FLASK_RUN_HOST=0.0.0.0
ENV FLASK_RUN_PORT=80

# Command to run Flask
CMD ["flask", "run", "--debug"]
