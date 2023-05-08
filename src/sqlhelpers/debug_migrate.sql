
--- INSERT INTO temp_table (vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year)
--- SELECT vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year
--- FROM (
---     SELECT *
---     FROM cars
---     WHERE id > (SELECT IFNULL(MAX(id), 0) AS last_processed_id FROM temp_table)
---     AND serial_number NOT IN (SELECT serial_number FROM temp_table) 
---     order by id asc
---     limit 10000
--- ) 


INSERT INTO temp_table (vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year)
SELECT vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year
FROM (
    SELECT *,
           RANK() OVER (PARTITION BY serial_number ORDER BY id DESC) AS rank_within_group
    FROM cars
    WHERE id > (SELECT IFNULL(MAX(id), 0) AS last_processed_id FROM temp_table)
    AND serial_number NOT IN (SELECT serial_number FROM temp_table)
) subquery
WHERE rank_within_group = 1  -- Select only rows with the highest id within each group
ORDER BY id ASC
LIMIT 7000;
