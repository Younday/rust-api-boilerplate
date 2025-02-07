db:
	docker-compose up -d
	
db-down:
	docker-compose down

migrate-up:
	sqlx migrate run

migrate-down:
	sqlx migrate revert

start-server:
	cargo watch -q -c -w src/ -x run