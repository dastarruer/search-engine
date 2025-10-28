FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .

RUN pip install --no-cache-dir -r requirements.txt

# Copy app code
COPY . .

# Tell flask where to find app
ENV FLASK_APP=main

# Tell flash where to host the server
ENV FLASK_RUN_HOST=0.0.0.0

# Command to run Flask
CMD ["flask", "run"]
