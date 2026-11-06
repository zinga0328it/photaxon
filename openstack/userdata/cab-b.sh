#!/bin/sh
exec >/dev/console 2>&1

probe() {
	label=$1
	target=$2
	expected=$3
	tries=0
	while [ "$tries" -lt 30 ]; do
		ping -c 1 -W 1 "$target" >/dev/null 2>&1 && icmp=PASS || icmp=BLOCKED
		nc -z -w 2 "$target" 22 >/dev/null 2>&1 && tcp=PASS || tcp=BLOCKED
		if [ "$icmp" = "$expected" ] && [ "$tcp" = "$expected" ]; then
			break
		fi
		tries=$((tries + 1))
		sleep 2
	done
	if [ "$icmp" = "$expected" ] && [ "$tcp" = "$expected" ]; then
		result=PASS
	else
		result=FAIL
	fi
	echo "FS_TEST $label ICMP=$icmp TCP=$tcp RESULT=$result"
}

probe CAB_B_TO_CENTRAL 10.91.12.10 PASS
probe CAB_B_TO_CAB_A 10.91.10.20 BLOCKED
probe CAB_B_TO_CAB_C 10.91.14.20 BLOCKED