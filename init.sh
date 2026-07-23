sudo service docker start
docker compose up -d
sqlx migrate run

docker exec -it $(docker compose ps -q postgres) psql -U network_gateway_user -d network_gateway_db -c "\dt"