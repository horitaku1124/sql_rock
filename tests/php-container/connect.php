<?php

declare(strict_types=1);

$host = getenv('DB_HOST') ?: '127.0.0.1';
$port = getenv('DB_PORT') ?: '13307';
$dsn = "mysql:host={$host};port={$port};dbname=sqlrock;charset=utf8mb4";

$pdo = new PDO($dsn, 'sqlrock', 'sqlrock', [
    PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION,
    PDO::ATTR_EMULATE_PREPARES => false,
]);

$pdo->exec('USE `sqlrock`');
$pdo->exec("SET NAMES 'utf8mb4' COLLATE 'utf8mb4_unicode_ci', SESSION sql_mode='STRICT_TRANS_TABLES'");

$existsQuery = <<<'SQL'
select exists (
    select 1 from information_schema.tables
    where table_schema = schema()
      and table_name = 'users'
      and table_type in ('BASE TABLE', 'SYSTEM VERSIONED')
) as `exists`
SQL;
if ((string) $pdo->query($existsQuery)->fetchColumn() !== '0') {
    throw new RuntimeException('users table unexpectedly exists');
}

$pdo->exec(<<<'SQL'
CREATE TABLE `users` (
    `id` BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name` VARCHAR(255) NOT NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE 'utf8mb4_unicode_ci'
SQL);
$pdo->exec('ALTER TABLE `users` ADD UNIQUE `users_name_unique`(`name`)');
$pdo->exec('ALTER TABLE `users` ADD INDEX `users_id_index`(`id`)');

if ((string) $pdo->query($existsQuery)->fetchColumn() !== '1') {
    throw new RuntimeException('users table was not found through information_schema');
}

$insert = $pdo->prepare('INSERT INTO users (id, name) VALUES (?, ?)');
$insert->execute([123, 'Alice']);

$row = $pdo->query('SELECT * FROM users WHERE id = 123')->fetch(PDO::FETCH_ASSOC);
$expected = ['id' => '123', 'name' => 'Alice'];

if ($row !== $expected) {
    fwrite(STDERR, "Unexpected query result: " . var_export($row, true) . PHP_EOL);
    exit(1);
}

$pdo->exec('DROP TABLE IF EXISTS users');
$pdo->exec('DROP TABLE IF EXISTS users');

echo "PHP container connected to sql_rock_server successfully" . PHP_EOL;
