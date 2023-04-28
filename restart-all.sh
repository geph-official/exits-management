#!/bin/sh

HOSTS=$(python3 -c "import yaml;import sys;data = yaml.safe_load(sys.stdin);print('\n'.join([entry['user'] + '@' + host for host, entry in data.items()]))" < exits.yaml)

# echo "$HOSTS"

for host in $HOSTS
do
echo "restarting $host..."
ssh -o StrictHostKeyChecking=no $host sudo service geph4-exit restart &
done; wait;
