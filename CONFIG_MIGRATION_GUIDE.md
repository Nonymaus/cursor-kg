# Configuration Migration Guide

## Overview

This guide helps you migrate from the old configuration format to the new consolidated configuration system introduced in version 2.0.

## What Changed

### Consolidated Configuration Files
- **Before**: `config.toml` and `enhanced_config.toml` (duplicated settings)
- **After**: Single `config.toml` with all settings

### New Security Section
A new `[security]` section has been added with comprehensive security settings:

```toml
[security]
enable_authentication = false
api_key = ""
admin_operations_require_auth = true
rate_limit_requests_per_minute = 60
rate_limit_burst = 10
max_content_length = 1048576
max_query_length = 1024
max_path_length = 4096
max_array_size = 1000
enable_encryption = false
encryption_key_file = ""
```

## Migration Steps

### Step 1: Backup Current Configuration
```bash
cp config.toml config.toml.backup
cp enhanced_config.toml enhanced_config.toml.backup  # if it exists
```

### Step 2: Update Configuration Format

If you have custom settings in your old configuration, add the new security section:

```toml
# Add this section to your existing config.toml
[security]
enable_authentication = false  # Set to true if you want API key auth
api_key = ""                   # Set your API key here
admin_operations_require_auth = true
rate_limit_requests_per_minute = 60
rate_limit_burst = 10
max_content_length = 1048576
max_query_length = 1024
max_path_length = 4096
max_array_size = 1000
enable_encryption = false
encryption_key_file = ""
```

### Step 3: Remove Duplicate Files
```bash
rm enhanced_config.toml  # If it exists
```

### Step 4: Validate Configuration
```bash
kg-mcp-server --validate-config
```

## Security Configuration Options

### Authentication
- `enable_authentication`: Enable/disable API key authentication
- `api_key`: Your API key (required if authentication is enabled)
- `admin_operations_require_auth`: Require auth for admin operations

### Rate Limiting
- `rate_limit_requests_per_minute`: Maximum requests per minute per client
- `rate_limit_burst`: Maximum burst requests in 10 seconds

### Input Validation
- `max_content_length`: Maximum content length (1MB default)
- `max_query_length`: Maximum query length (1KB default)
- `max_path_length`: Maximum file path length (4KB default)
- `max_array_size`: Maximum array size (1000 default)

### Encryption (Future Feature)
- `enable_encryption`: Enable data encryption at rest
- `encryption_key_file`: Path to encryption key file

## Backward Compatibility

The new configuration system is backward compatible:
- Old configuration files without the security section will use default security settings
- Authentication is disabled by default
- All existing settings continue to work

## Troubleshooting

### Configuration Validation Errors

**Error**: "API key must be set when authentication is enabled"
**Solution**: Set `api_key` in the security section or disable authentication

**Error**: "Rate limit requests per minute must be greater than 0"
**Solution**: Set a positive value for `rate_limit_requests_per_minute`

### Migration Issues

**Issue**: Server won't start after migration
**Solution**: 
1. Check configuration syntax with `kg-mcp-server --validate-config`
2. Restore from backup if needed
3. Compare with the default configuration

**Issue**: Authentication not working
**Solution**:
1. Ensure `enable_authentication = true`
2. Set a valid `api_key`
3. Pass the API key in requests via `Authorization: Bearer <api_key>` header

## Example Configurations

### Development (No Authentication)
```toml
[security]
enable_authentication = false
admin_operations_require_auth = false
rate_limit_requests_per_minute = 1000
rate_limit_burst = 100
```

### Production (With Authentication)
```toml
[security]
enable_authentication = true
api_key = "your-secure-api-key-here"
admin_operations_require_auth = true
rate_limit_requests_per_minute = 60
rate_limit_burst = 10
```

### High Security
```toml
[security]
enable_authentication = true
api_key = "your-secure-api-key-here"
admin_operations_require_auth = true
rate_limit_requests_per_minute = 30
rate_limit_burst = 5
max_content_length = 524288  # 512KB
max_query_length = 512
max_array_size = 100
```

## Testing Your Configuration

After migration, test your configuration:

```bash
# Validate configuration
kg-mcp-server --validate-config

# Test server startup
kg-mcp-server --dry-run

# Test with authentication (if enabled)
curl -H "Authorization: Bearer your-api-key" http://localhost:8360/health
```

## Support

If you encounter issues during migration:
1. Check the troubleshooting section above
2. Validate your configuration syntax
3. Review the server logs for detailed error messages
4. Restore from backup if needed

For additional help, refer to the main documentation or create an issue in the project repository.
