SELECT json_build_object(
    'time', time,
    'outdoor_temp', outdoor_temp,
    'humidity', humidity,
    'uvi', uvi,
    'rain_rate', rain_rate
) FROM weather ORDER BY time DESC LIMIT 1;

WITH PressureBuckets AS (
    SELECT
        time_bucket('10 minutes', time) as bucket,
        AVG(pressure) as avg_pressure
    FROM weather
    WHERE time >= NOW() - INTERVAL '2 hours'
    GROUP BY bucket
    ORDER BY bucket DESC
)
SELECT json_build_object(
    'pressure_difference', (SELECT avg_pressure FROM PressureBuckets ORDER BY bucket DESC LIMIT 1) - 
        (SELECT avg_pressure FROM PressureBuckets ORDER BY bucket DESC OFFSET 6 LIMIT 1)
) FROM PressureBuckets
LIMIT 1;
