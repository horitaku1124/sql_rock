FROM php:8.3-cli

RUN docker-php-ext-install pdo_mysql

COPY connect.php /usr/local/bin/sql-rock-php-connect.php

ENTRYPOINT ["php", "/usr/local/bin/sql-rock-php-connect.php"]
