SELECT * FROM Cars
WHERE EXISTS (
  SELECT 1 FROM Cars c2 
  WHERE Cars.serial_number = c2.serial_number
);
